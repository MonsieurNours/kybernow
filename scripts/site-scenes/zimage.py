#!/usr/bin/env python3
"""Client minimal ComfyUI pour Z-Image Turbo (réglages du flux « Image » de l'auteur) :

    zimage.py NOM "prompt" [--w 1536] [--h 864] [--seed 1] [--n 1]

Écrit NOM-<seed>.png dans le dossier courant. Serveur : $COMFY (http://192.168.1.100:8188).
"""
import argparse, json, os, sys, time, urllib.parse, urllib.request, uuid

COMFY = os.environ.get("COMFY", "http://192.168.1.100:8188")
NEG = ("low quality, blurry, bad anatomy, deformed hands, extra fingers, missing fingers, extra limbs, "
       "text, watermark, logo, signature, cartoon, illustration, 3d render, oversaturated")


def call(path, data=None):
    req = urllib.request.Request(COMFY + path, data=json.dumps(data).encode() if data is not None else None,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def graph(prompt, w, h, seed, prefix):
    return {
        "62": {"class_type": "CLIPLoader", "inputs": {"clip_name": "qwen_3_4b_fp8_mixed.safetensors", "type": "lumina2", "device": "default"}},
        "63": {"class_type": "VAELoader", "inputs": {"vae_name": "ae.safetensors"}},
        "66": {"class_type": "UNETLoader", "inputs": {"unet_name": "z_image_turbo_int8_convrot.safetensors", "weight_dtype": "default"}},
        "69": {"class_type": "ModelSamplingAuraFlow", "inputs": {"model": ["66", 0], "shift": 3}},
        "67": {"class_type": "CLIPTextEncode", "inputs": {"clip": ["62", 0], "text": prompt}},
        "71": {"class_type": "CLIPTextEncode", "inputs": {"clip": ["62", 0], "text": NEG}},
        "68": {"class_type": "EmptySD3LatentImage", "inputs": {"width": w, "height": h, "batch_size": 1}},
        "70": {"class_type": "KSampler", "inputs": {"model": ["69", 0], "seed": seed, "steps": 8, "cfg": 1,
                                                     "sampler_name": "res_multistep", "scheduler": "simple", "denoise": 1,
                                                     "positive": ["67", 0], "negative": ["71", 0], "latent_image": ["68", 0]}},
        "65": {"class_type": "VAEDecode", "inputs": {"samples": ["70", 0], "vae": ["63", 0]}},
        "9": {"class_type": "SaveImage", "inputs": {"filename_prefix": prefix, "images": ["65", 0]}},
    }


def run(name, prompt, w, h, seed):
    client = str(uuid.uuid4())
    r = json.loads(call("/prompt", {"prompt": graph(prompt, w, h, seed, "kybernow-site/" + name), "client_id": client}))
    pid = r["prompt_id"]
    t0 = time.time()
    while True:
        time.sleep(1.5)
        hist = json.loads(call("/history/" + pid))
        if pid in hist:
            out = hist[pid]
            if out.get("status", {}).get("status_str") == "error":
                sys.exit("erreur ComfyUI : " + json.dumps(out["status"])[:800])
            imgs = [i for n in out["outputs"].values() for i in n.get("images", [])]
            if imgs:
                break
        if time.time() - t0 > 600:
            sys.exit("délai dépassé")
    img = imgs[0]
    q = urllib.parse.urlencode({"filename": img["filename"], "subfolder": img["subfolder"], "type": img["type"]})
    data = urllib.request.urlopen(COMFY + "/view?" + q, timeout=60).read()
    path = f"{name}-{seed}.png"
    open(path, "wb").write(data)
    print(f"{path} ({time.time() - t0:.0f} s)")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("name"); ap.add_argument("prompt")
    ap.add_argument("--w", type=int, default=1536); ap.add_argument("--h", type=int, default=864)
    ap.add_argument("--seed", type=int, default=1); ap.add_argument("--n", type=int, default=1)
    a = ap.parse_args()
    for s in range(a.seed, a.seed + a.n):
        run(a.name, a.prompt, a.w, a.h, s)
