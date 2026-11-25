// Essai hors ligne du décodage PyroWave d'Android (apps/android/tests/pyrowave/run.sh),
// sur un appareil ou un émulateur, sans ordinateur distant :
//
//   kn_pyrowave_test probe
//       la sonde du lancement (kn_pw_probe)
//   kn_pyrowave_test encode src.yuv out.pwv W H 420|444 Mbps
//       encode une source YUV 8 bits plage réduite (ffmpeg -pix_fmt yuv420p|yuv444p)
//       avec l'encodeur de l'amont (API C de PyroWave, device Vulkan par défaut),
//       comme la Station : une image = les paquets de pyrowave_encoder_packetize
//       (frontière de 1 152 octets) bout à bout
//   kn_pyrowave_test decode in.pwv src.yuv [hdr]
//       décode et présente chaque image par le vrai chemin de l'app (kn_pw_open,
//       kn_pw_configure, kn_pw_frame) sur une surface AImageReader (RGBA 8 bits),
//       cadencé à 60 i/s ; relit la première image présentée et la compare (PSNR
//       en RGB) à la source convertie en BT.709 ; vérifie qu'une image tronquée
//       est sautée et que la suivante se décode. « hdr » : même flux étiqueté HDR10
//       (plans R16, conversion PQ → SDR, la surface AImageReader restant SDR).
//
// Fichier .pwv : "KNPWTEST" | largeur u32 | hauteur u32 | 4:4:4 u32 | images
// (taille u32 LE puis octets).
#include "volk.h"
#include "pyrowave.h"
#include "kn_pyrowave.h"

#include <android/hardware_buffer.h>
#include <media/NdkImageReader.h>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <thread>
#include <vector>

using Clock = std::chrono::steady_clock;

static std::vector<uint8_t> read_file(const char *path)
{
	std::vector<uint8_t> v;
	FILE *f = fopen(path, "rb");
	if (!f)
		return v;
	fseek(f, 0, SEEK_END);
	v.resize(size_t(ftell(f)));
	fseek(f, 0, SEEK_SET);
	if (fread(v.data(), 1, v.size(), f) != v.size())
		v.clear();
	fclose(f);
	return v;
}

static int probe()
{
	char report[256] = {};
	auto t0 = Clock::now();
	int ok = kn_pw_probe(report, sizeof(report));
	double ms = std::chrono::duration<double, std::milli>(Clock::now() - t0).count();
	printf("Sonde : %s (%s) en %.0f ms\n", ok ? "PyroWave disponible" : "PyroWave indisponible", report, ms);
	return ok ? 0 : 2;
}

static int encode(const char *src, const char *out, int w, int h, bool c444, int mbps)
{
	auto yuv = read_file(src);
	size_t csize = c444 ? size_t(w) * h : size_t(w / 2) * (h / 2);
	size_t fsize = size_t(w) * h + 2 * csize;
	if (yuv.size() < fsize)
	{
		fprintf(stderr, "source trop courte\n");
		return 1;
	}
	pyrowave_device dev = nullptr;
	pyrowave_result r = pyrowave_create_default_device(&dev);
	if (r != PYROWAVE_SUCCESS)
	{
		fprintf(stderr, "encodeur : pas de device Vulkan (%d)\n", int(r));
		return 3;
	}
	pyrowave_encoder_create_info ei = { dev, w, h, c444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420 };
	pyrowave_encoder enc = nullptr;
	if ((r = pyrowave_encoder_create(&ei, &enc)) != PYROWAVE_SUCCESS)
	{
		fprintf(stderr, "encodeur refusé par ce GPU (%d)\n", int(r));
		pyrowave_device_destroy(dev);
		return 3;
	}
	FILE *f = fopen(out, "wb");
	uint32_t head[3] = { uint32_t(w), uint32_t(h), c444 ? 1u : 0u };
	fwrite("KNPWTEST", 1, 8, f);
	fwrite(head, 4, 3, f);
	size_t frames = yuv.size() / fsize, total = 0;
	pyrowave_rate_control rc = { (size_t(mbps) * 1000000 / 8 / 60) & ~size_t(3) };
	std::vector<uint8_t> bits(rc.maximum_bitstream_size + 65536);
	std::vector<pyrowave_packet> packets;
	for (size_t i = 0; i < frames; i++)
	{
		uint8_t *base = yuv.data() + i * fsize;
		pyrowave_cpu_buffer buf = {};
		buf.data[0] = base;
		buf.data[1] = base + size_t(w) * h;
		buf.data[2] = base + size_t(w) * h + csize;
		buf.row_stride_in_bytes[0] = size_t(w);
		buf.row_stride_in_bytes[1] = buf.row_stride_in_bytes[2] = c444 ? size_t(w) : size_t(w / 2);
		buf.plane_size_in_bytes[0] = size_t(w) * h;
		buf.plane_size_in_bytes[1] = buf.plane_size_in_bytes[2] = csize;
		buf.width = w;
		buf.height = h;
		buf.format = c444 ? PYROWAVE_CPU_BUFFER_FORMAT_YUV444P : PYROWAVE_CPU_BUFFER_FORMAT_YUV420P;
		if ((r = pyrowave_encoder_encode_cpu(enc, &buf, &rc)) != PYROWAVE_SUCCESS)
		{
			fprintf(stderr, "image %zu non encodée (%d)\n", i, int(r));
			return 3;
		}
		size_t n = 0, got = 0;
		pyrowave_encoder_compute_num_packets(enc, 1152, &n);
		packets.resize(n);
		pyrowave_encoder_packetize(enc, packets.data(), 1152, &got, bits.data(), bits.size());
		// Une image = ses paquets bout à bout (paquet média de la Station).
		std::vector<uint8_t> frame;
		for (size_t p = 0; p < got; p++)
			frame.insert(frame.end(), bits.data() + packets[p].offset, bits.data() + packets[p].offset + packets[p].size);
		uint32_t size = uint32_t(frame.size());
		fwrite(&size, 4, 1, f);
		fwrite(frame.data(), 1, frame.size(), f);
		total += frame.size();
	}
	fclose(f);
	pyrowave_encoder_destroy(enc);
	pyrowave_device_destroy(dev);
	printf("Encodé : %zu images %dx%d %s, %.0f Mbit/s à 60 i/s\n", frames, w, h, c444 ? "4:4:4" : "4:2:0",
	       double(total) * 8.0 * 60.0 / double(frames) / 1e6);
	return 0;
}

// BT.709 plage réduite → RGB 8 bits (la référence de la comparaison).
static void reference_rgb(int y, int cb, int cr, double out[3])
{
	double Y = (y - 16) / 219.0, Cb = (cb - 128) / 224.0, Cr = (cr - 128) / 224.0;
	double kr = 0.2126, kb = 0.0722, kg = 1 - kr - kb;
	double r = Y + (2 - 2 * kr) * Cr, b = Y + (2 - 2 * kb) * Cb, g = (Y - kr * r - kb * b) / kg;
	out[0] = std::clamp(r, 0.0, 1.0) * 255;
	out[1] = std::clamp(g, 0.0, 1.0) * 255;
	out[2] = std::clamp(b, 0.0, 1.0) * 255;
}

static int decode(const char *in, const char *src, bool hdr)
{
	auto file = read_file(in);
	if (file.size() < 20 || memcmp(file.data(), "KNPWTEST", 8) != 0)
	{
		fprintf(stderr, "fichier .pwv illisible\n");
		return 1;
	}
	uint32_t w, h, c444;
	memcpy(&w, &file[8], 4);
	memcpy(&h, &file[12], 4);
	memcpy(&c444, &file[16], 4);
	std::vector<std::pair<size_t, size_t>> frames;
	for (size_t at = 20; at + 4 <= file.size();)
	{
		uint32_t n;
		memcpy(&n, &file[at], 4);
		frames.push_back({ at + 4, n });
		at += 4 + n;
	}
	auto yuv = read_file(src);

	char report[256] = {};
	if (!kn_pw_probe(report, sizeof(report)))
	{
		printf("PyroWave indisponible : %s\n", report);
		return 2;
	}
	AImageReader *reader = nullptr;
	if (AImageReader_newWithUsage(int32_t(w), int32_t(h), AIMAGE_FORMAT_RGBA_8888,
	                              AHARDWAREBUFFER_USAGE_GPU_COLOR_OUTPUT | AHARDWAREBUFFER_USAGE_CPU_READ_OFTEN, 4, &reader) != AMEDIA_OK)
	{
		fprintf(stderr, "AImageReader impossible\n");
		return 1;
	}
	ANativeWindow *window = nullptr;
	AImageReader_getWindow(reader, &window);
	char err[256] = {};
	kn_pw_session *s = kn_pw_open(window, err, sizeof(err));
	if (!s || !kn_pw_configure(s, w, h, int(c444), hdr ? 1 : 0, 0, 0, 0, 0))
	{
		fprintf(stderr, "session impossible : %s\n", err);
		return 1;
	}
	printf("%s, %ux%u %s%s\n", kn_pw_describe(s), w, h, c444 ? "4:4:4" : "4:2:0", hdr ? ", étiqueté HDR10" : "");

	// Première image, relue et comparée à la source.
	int status = 0;
	if (kn_pw_frame(s, &file[frames[0].first], frames[0].second) != KN_PW_SHOWN)
	{
		fprintf(stderr, "première image non montrée\n");
		return 1;
	}
	AImage *image = nullptr;
	for (int i = 0; i < 50 && AImageReader_acquireLatestImage(reader, &image) != AMEDIA_OK; i++)
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
	if (!image)
	{
		fprintf(stderr, "aucune image présentée sur la surface\n");
		return 1;
	}
	uint8_t *px = nullptr;
	int len = 0, stride = 0;
	AImage_getPlaneData(image, 0, &px, &len);
	AImage_getPlaneRowStride(image, 0, &stride);
	if (!hdr && yuv.size() >= size_t(w) * h)
	{
		size_t cw = c444 ? w : w / 2, csize = c444 ? size_t(w) * h : size_t(w / 2) * (h / 2);
		const uint8_t *Y = yuv.data(), *Cb = Y + size_t(w) * h, *Cr = Cb + csize;
		double se = 0;
		for (uint32_t y = 0; y < h; y++)
			for (uint32_t x = 0; x < w; x++)
			{
				size_t ci = c444 ? size_t(y) * w + x : size_t(y / 2) * cw + x / 2;
				double ref[3];
				reference_rgb(Y[size_t(y) * w + x], Cb[ci], Cr[ci], ref);
				const uint8_t *p = px + size_t(y) * stride + x * 4;
				for (int c = 0; c < 3; c++)
					se += (p[c] - ref[c]) * (p[c] - ref[c]);
			}
		double mse = se / (double(w) * h * 3);
		double psnr = 10 * std::log10(255.0 * 255.0 / std::max(mse, 1e-9));
		printf("Première image : PSNR %.1f dB en RGB contre la source\n", psnr);
		if (psnr < 30)
		{
			fprintf(stderr, "image décodée trop loin de la source\n");
			status = 1;
		}
	}
	else
	{
		// HDR étiqueté sur une source SDR : seulement plausible (ni noire ni blanche).
		double sum = 0;
		for (uint32_t y = 0; y < h; y += 8)
			for (uint32_t x = 0; x < w; x += 8)
				sum += px[size_t(y) * stride + x * 4 + 1];
		double mean = sum / ((h / 8.0) * (w / 8.0));
		printf("Première image (HDR10 → SDR) : vert moyen %.0f sur 255\n", mean);
		if (mean < 5 || mean > 250)
			status = 1;
	}
	AImage_delete(image);

	// Image tronquée : sautée, sans casser la suivante.
	int cut = kn_pw_frame(s, &file[frames[0].first], frames[0].second / 3);
	printf("Image tronquée au tiers : %s\n", cut == KN_PW_INCOMPLETE ? "sautée" : "montrée (attendu : sautée)");
	if (cut != KN_PW_INCOMPLETE)
		status = 1;

	// Toutes les images, cadencées à 60 i/s : temps de décodage et présentation.
	std::vector<double> times;
	auto next = Clock::now();
	for (size_t i = 0; i < frames.size(); i++)
	{
		std::this_thread::sleep_until(next);
		next += std::chrono::microseconds(16667);
		auto t0 = Clock::now();
		int r = kn_pw_frame(s, &file[frames[i].first], frames[i].second);
		times.push_back(std::chrono::duration<double, std::milli>(Clock::now() - t0).count());
		if (r != KN_PW_SHOWN)
		{
			fprintf(stderr, "image %zu : %d\n", i, r);
			status = 1;
		}
		AImage *img = nullptr;
		while (AImageReader_acquireLatestImage(reader, &img) == AMEDIA_OK && img)
		{
			AImage_delete(img);
			img = nullptr;
		}
	}
	std::vector<double> sorted = times;
	std::sort(sorted.begin(), sorted.end());
	double mean = 0;
	for (double t : times)
		mean += t;
	mean /= double(times.size());
	printf("%zu images cadencées à 60 i/s : décodage et présentation %.2f ms en moyenne (médiane %.2f ; maximum %.2f)\n",
	       times.size(), mean, sorted[sorted.size() / 2], sorted.back());
	kn_pw_close(s);
	AImageReader_delete(reader);
	printf("%s\n", status ? "ÉCHEC" : "OK");
	return status;
}

int main(int argc, char **argv)
{
	if (argc >= 2 && !strcmp(argv[1], "probe"))
		return probe();
	if (argc >= 8 && !strcmp(argv[1], "encode"))
		return encode(argv[2], argv[3], atoi(argv[4]), atoi(argv[5]), !strcmp(argv[6], "444"), atoi(argv[7]));
	if (argc >= 4 && !strcmp(argv[1], "decode"))
		return decode(argv[2], argv[3], argc >= 5 && !strcmp(argv[4], "hdr"));
	fprintf(stderr, "kn_pyrowave_test probe | encode src.yuv out.pwv W H 420|444 Mbps | decode in.pwv src.yuv [hdr]\n");
	return 1;
}
