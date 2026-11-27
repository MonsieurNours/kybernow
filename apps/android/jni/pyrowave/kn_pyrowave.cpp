// Décodage PyroWave de l'app Android : voir kn_pyrowave.h et
// docs/pyrowave-client-android.md.
//
// Un seul device Vulkan pour l'app (créé par la sonde du lancement, gardé),
// prêté à PyroWave (pyrowave_create_device) : une file graphique et de calcul,
// qui présente aussi. Chaque image d'une session s'enregistre dans un seul
// tampon de commandes : le décodage de PyroWave (pyrowave_device_set_command_buffer),
// puis la conversion YCbCr → RGB dans l'image de la swapchain, soumis puis
// présenté ; la fonction rend la main GPU terminé (une image à la fois : les
// tampons internes de PyroWave ne sont jamais réutilisés en cours de route).
//
// Vulkan est chargé par volk (celui de Granite) : rien n'est lié à libvulkan.so,
// l'app démarre sur un appareil sans Vulkan. Tous les appels passent sous un
// même verrou (Granite n'est pas fait pour deux fils à la fois).

#define VK_USE_PLATFORM_ANDROID_KHR
#include "volk.h"
#include "pyrowave.h"
#include "kn_pyrowave.h"

#include <android/log.h>
#include <android/native_window.h>

#include <algorithm>
#include <cmath>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <string>
#include <vector>

#define LOG_TAG "KyberNow"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, LOG_TAG, __VA_ARGS__)

namespace
{
const uint32_t present_vert[] = {
#include "present.vert.inc"
};
const uint32_t present_frag[] = {
#include "present.frag.inc"
};
const uint8_t probe_frame[] = {
#include "probe_frame.inc"
};

std::mutex lock;

void put(char *out, size_t n, const char *fmt, ...)
{
	if (!out || !n)
		return;
	va_list ap;
	va_start(ap, fmt);
	vsnprintf(out, n, fmt, ap);
	va_end(ap);
}

bool has_ext(const std::vector<VkExtensionProperties> &list, const char *name)
{
	return std::any_of(list.begin(), list.end(), [&](const VkExtensionProperties &e) { return strcmp(e.extensionName, name) == 0; });
}

// Le device de l'app. Les create infos restent en vie avec lui (Granite les relit).
struct Gpu
{
	VkInstance instance = VK_NULL_HANDLE;
	VkPhysicalDevice physical = VK_NULL_HANDLE;
	VkDevice device = VK_NULL_HANDLE;
	VkQueue queue = VK_NULL_HANDLE;
	uint32_t family = 0;
	pyrowave_device pyro = nullptr;
	bool fragment = false;
	bool swapchain = false;
	bool colorspace = false;
	bool hdr_metadata = false;
	std::string name;
	PFN_vkCreateAndroidSurfaceKHR create_surface = nullptr;

	VkApplicationInfo app = { VK_STRUCTURE_TYPE_APPLICATION_INFO };
	std::vector<const char *> instance_exts;
	VkInstanceCreateInfo instance_info = { VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO };
	std::vector<const char *> device_exts;
	float priority = 1.0f;
	VkDeviceQueueCreateInfo queue_info = { VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO };
	VkPhysicalDeviceVulkan13Features f13 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES };
	VkPhysicalDeviceVulkan12Features f12 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES };
	VkPhysicalDeviceVulkan11Features f11 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_1_FEATURES };
	VkPhysicalDeviceFeatures2 features = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2 };
	VkDeviceCreateInfo device_info = { VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO };
	pyrowave_device_create_queue_info pyro_queue = {};
};

Gpu *gpu_instance = nullptr;
std::string gpu_error;
bool gpu_tried = false;

// Le device de l'app, créé au premier appel (sous le verrou). Nul si l'appareil
// n'a pas ce qu'il faut ; la raison dans gpu_error.
Gpu *gpu()
{
	if (gpu_tried)
		return gpu_instance;
	gpu_tried = true;
	auto fail = [](const char *why) -> Gpu * {
		gpu_error = why;
		LOGW("PyroWave : %s", why);
		return nullptr;
	};

	if (volkInitialize() != VK_SUCCESS)
		return fail("no Vulkan loader");
	uint32_t loader = VK_API_VERSION_1_0;
	if (vkEnumerateInstanceVersion)
		vkEnumerateInstanceVersion(&loader);
	if (loader < VK_API_VERSION_1_3)
	{
		char b[96];
		snprintf(b, sizeof(b), "Vulkan loader %u.%u, 1.3 needed", VK_API_VERSION_MAJOR(loader), VK_API_VERSION_MINOR(loader));
		gpu_error = b;
		LOGW("PyroWave : %s", b);
		return nullptr;
	}

	auto *g = new Gpu();
	uint32_t n = 0;
	vkEnumerateInstanceExtensionProperties(nullptr, &n, nullptr);
	std::vector<VkExtensionProperties> iexts(n);
	vkEnumerateInstanceExtensionProperties(nullptr, &n, iexts.data());
	bool surface = has_ext(iexts, VK_KHR_SURFACE_EXTENSION_NAME) && has_ext(iexts, VK_KHR_ANDROID_SURFACE_EXTENSION_NAME);
	if (surface)
	{
		g->instance_exts.push_back(VK_KHR_SURFACE_EXTENSION_NAME);
		g->instance_exts.push_back(VK_KHR_ANDROID_SURFACE_EXTENSION_NAME);
		if (has_ext(iexts, VK_EXT_SWAPCHAIN_COLOR_SPACE_EXTENSION_NAME))
		{
			g->instance_exts.push_back(VK_EXT_SWAPCHAIN_COLOR_SPACE_EXTENSION_NAME);
			g->colorspace = true;
		}
	}
	g->app.pApplicationName = "KyberNow";
	g->app.pEngineName = "KyberNow";
	g->app.apiVersion = VK_API_VERSION_1_3;
	g->instance_info.pApplicationInfo = &g->app;
	g->instance_info.enabledExtensionCount = uint32_t(g->instance_exts.size());
	g->instance_info.ppEnabledExtensionNames = g->instance_exts.data();
	if (vkCreateInstance(&g->instance_info, nullptr, &g->instance) != VK_SUCCESS)
	{
		delete g;
		return fail("vkCreateInstance failed");
	}
	volkLoadInstance(g->instance);
	if (surface)
		g->create_surface = reinterpret_cast<PFN_vkCreateAndroidSurfaceKHR>(vkGetInstanceProcAddr(g->instance, "vkCreateAndroidSurfaceKHR"));

	vkEnumeratePhysicalDevices(g->instance, &n, nullptr);
	std::vector<VkPhysicalDevice> gpus(n);
	vkEnumeratePhysicalDevices(g->instance, &n, gpus.data());
	std::string why = "no GPU";
	for (auto pd : gpus)
	{
		VkPhysicalDeviceVulkan11Properties p11 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES };
		VkPhysicalDeviceProperties2 props = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_PROPERTIES_2, &p11 };
		vkGetPhysicalDeviceProperties2(pd, &props);
		const char *name = props.properties.deviceName;
		if (props.properties.apiVersion < VK_API_VERSION_1_3)
		{
			why = std::string(name) + ": Vulkan " + std::to_string(VK_API_VERSION_MAJOR(props.properties.apiVersion)) + "." +
			      std::to_string(VK_API_VERSION_MINOR(props.properties.apiVersion)) + ", 1.3 needed";
			continue;
		}
		const VkSubgroupFeatureFlags ops = VK_SUBGROUP_FEATURE_BASIC_BIT | VK_SUBGROUP_FEATURE_VOTE_BIT | VK_SUBGROUP_FEATURE_BALLOT_BIT |
		                                   VK_SUBGROUP_FEATURE_ARITHMETIC_BIT | VK_SUBGROUP_FEATURE_SHUFFLE_BIT |
		                                   VK_SUBGROUP_FEATURE_SHUFFLE_RELATIVE_BIT;
		if ((p11.subgroupSupportedOperations & ops) != ops || !(p11.subgroupSupportedStages & VK_SHADER_STAGE_COMPUTE_BIT))
		{
			why = std::string(name) + ": missing subgroup operations";
			continue;
		}
		g->f13 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_3_FEATURES };
		g->f12 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_2_FEATURES, &g->f13 };
		g->f11 = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_VULKAN_1_1_FEATURES, &g->f12 };
		g->features = { VK_STRUCTURE_TYPE_PHYSICAL_DEVICE_FEATURES_2, &g->f11 };
		vkGetPhysicalDeviceFeatures2(pd, &g->features);
		if (!g->f13.subgroupSizeControl || !g->f13.computeFullSubgroups)
		{
			why = std::string(name) + ": no subgroup size control";
			continue;
		}
		if (!g->f13.synchronization2 || !g->f13.dynamicRendering)
		{
			why = std::string(name) + ": no synchronization2 or dynamic rendering";
			continue;
		}
		vkGetPhysicalDeviceQueueFamilyProperties(pd, &n, nullptr);
		std::vector<VkQueueFamilyProperties> families(n);
		vkGetPhysicalDeviceQueueFamilyProperties(pd, &n, families.data());
		uint32_t family = UINT32_MAX;
		for (uint32_t i = 0; i < n; i++)
			if ((families[i].queueFlags & (VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT)) == (VK_QUEUE_GRAPHICS_BIT | VK_QUEUE_COMPUTE_BIT))
			{
				family = i;
				break;
			}
		if (family == UINT32_MAX)
		{
			why = std::string(name) + ": no graphics and compute queue";
			continue;
		}
		g->physical = pd;
		g->family = family;
		g->name = name;
		break;
	}
	if (!g->physical)
	{
		vkDestroyInstance(g->instance, nullptr);
		delete g;
		gpu_error = why;
		LOGW("PyroWave : %s", why.c_str());
		return nullptr;
	}

	// Tout ce que le GPU propose, sauf ce qui coûte sans servir (robustesse,
	// mémoire protégée, ressources creuses).
	g->features.features.robustBufferAccess = VK_FALSE;
	g->features.features.sparseBinding = VK_FALSE;
	g->features.features.sparseResidencyBuffer = VK_FALSE;
	g->features.features.sparseResidencyImage2D = VK_FALSE;
	g->features.features.sparseResidencyImage3D = VK_FALSE;
	g->features.features.sparseResidency2Samples = VK_FALSE;
	g->features.features.sparseResidency4Samples = VK_FALSE;
	g->features.features.sparseResidency8Samples = VK_FALSE;
	g->features.features.sparseResidency16Samples = VK_FALSE;
	g->features.features.sparseResidencyAliased = VK_FALSE;
	g->f11.protectedMemory = VK_FALSE;
	g->f12.bufferDeviceAddressCaptureReplay = VK_FALSE;
	g->f12.bufferDeviceAddressMultiDevice = VK_FALSE;
	g->f13.robustImageAccess = VK_FALSE;

	vkEnumerateDeviceExtensionProperties(g->physical, nullptr, &n, nullptr);
	std::vector<VkExtensionProperties> dexts(n);
	vkEnumerateDeviceExtensionProperties(g->physical, nullptr, &n, dexts.data());
	if (g->create_surface && has_ext(dexts, VK_KHR_SWAPCHAIN_EXTENSION_NAME))
	{
		g->device_exts.push_back(VK_KHR_SWAPCHAIN_EXTENSION_NAME);
		g->swapchain = true;
		if (has_ext(dexts, VK_EXT_HDR_METADATA_EXTENSION_NAME))
		{
			g->device_exts.push_back(VK_EXT_HDR_METADATA_EXTENSION_NAME);
			g->hdr_metadata = true;
		}
	}
	g->queue_info.queueFamilyIndex = g->family;
	g->queue_info.queueCount = 1;
	g->queue_info.pQueuePriorities = &g->priority;
	g->device_info.pNext = &g->features;
	g->device_info.queueCreateInfoCount = 1;
	g->device_info.pQueueCreateInfos = &g->queue_info;
	g->device_info.enabledExtensionCount = uint32_t(g->device_exts.size());
	g->device_info.ppEnabledExtensionNames = g->device_exts.data();
	if (vkCreateDevice(g->physical, &g->device_info, nullptr, &g->device) != VK_SUCCESS)
	{
		vkDestroyInstance(g->instance, nullptr);
		delete g;
		return fail("vkCreateDevice failed");
	}
	volkLoadDevice(g->device);
	vkGetDeviceQueue(g->device, g->family, 0, &g->queue);

	g->pyro_queue = { g->queue, g->family, 0 };
	pyrowave_device_create_info info = {};
	info.GetInstanceProcAddr = vkGetInstanceProcAddr;
	info.instance = g->instance;
	info.physical_device = g->physical;
	info.device = g->device;
	info.instance_create_info = &g->instance_info;
	info.device_create_info = &g->device_info;
	info.queue_info = &g->pyro_queue;
	info.queue_info_count = 1;
	if (pyrowave_create_device(&info, &g->pyro) != PYROWAVE_SUCCESS)
	{
		// Le device reste à l'app (PyroWave ne l'a pas pris) : on le rend.
		vkDestroyDevice(g->device, nullptr);
		vkDestroyInstance(g->instance, nullptr);
		delete g;
		return fail("pyrowave_create_device failed");
	}
	// pyrowave_create_device recharge volk sur nos poignées : rien à refaire.
	g->fragment = pyrowave_decoder_device_prefers_fragment_path(g->pyro);
	LOGI("PyroWave : %s, Vulkan 1.3, %s path%s%s", g->name.c_str(), g->fragment ? "fragment" : "compute",
	     g->swapchain ? "" : ", no swapchain", g->colorspace ? ", HDR10 colour space" : "");
	gpu_instance = g;
	return g;
}

uint32_t memory_type(Gpu *g, uint32_t bits, VkMemoryPropertyFlags want)
{
	VkPhysicalDeviceMemoryProperties mp;
	vkGetPhysicalDeviceMemoryProperties(g->physical, &mp);
	for (uint32_t i = 0; i < mp.memoryTypeCount; i++)
		if ((bits & (1u << i)) && (mp.memoryTypes[i].propertyFlags & want) == want)
			return i;
	for (uint32_t i = 0; i < mp.memoryTypeCount; i++)
		if (bits & (1u << i))
			return i;
	return 0;
}

// Le format d'un plan : R16 pour le HDR si le GPU le prend pour l'usage du
// décodeur (sinon R8 : un peu de précision perdue, l'image reste juste).
bool plane_format_ok(Gpu *g, VkFormat f)
{
	VkFormatProperties fp;
	vkGetPhysicalDeviceFormatProperties(g->physical, f, &fp);
	VkFormatFeatureFlags need = VK_FORMAT_FEATURE_SAMPLED_IMAGE_BIT |
	                            (g->fragment ? VK_FORMAT_FEATURE_COLOR_ATTACHMENT_BIT : VK_FORMAT_FEATURE_STORAGE_IMAGE_BIT);
	return (fp.optimalTilingFeatures & need) == need;
}

bool linear_filter(Gpu *g, VkFormat f)
{
	VkFormatProperties fp;
	vkGetPhysicalDeviceFormatProperties(g->physical, f, &fp);
	return (fp.optimalTilingFeatures & VK_FORMAT_FEATURE_SAMPLED_IMAGE_FILTER_LINEAR_BIT) != 0;
}

struct Plane
{
	VkImage image = VK_NULL_HANDLE;
	VkDeviceMemory memory = VK_NULL_HANDLE;
	VkImageView view = VK_NULL_HANDLE;
	uint32_t width = 0, height = 0;
};

struct PushConstants
{
	float range[4];
	float coef[4];
	int32_t mode;
	float sdr_white;
};
} // namespace

struct kn_pw_session
{
	Gpu *g = nullptr;
	ANativeWindow *window = nullptr;
	VkSurfaceKHR surface = VK_NULL_HANDLE;

	// Swapchain.
	VkSwapchainKHR swapchain = VK_NULL_HANDLE;
	VkFormat format = VK_FORMAT_UNDEFINED;
	VkColorSpaceKHR colorspace = VK_COLOR_SPACE_SRGB_NONLINEAR_KHR;
	VkExtent2D extent = {};
	std::vector<VkImage> images;
	std::vector<VkImageView> views;
	std::vector<VkSemaphore> rendered;
	bool hdr_output = false;

	// Rendu.
	VkCommandPool pool = VK_NULL_HANDLE;
	VkCommandBuffer cmd = VK_NULL_HANDLE;
	VkFence fence = VK_NULL_HANDLE;
	VkSemaphore acquired = VK_NULL_HANDLE;
	VkSampler sampler = VK_NULL_HANDLE;
	VkDescriptorSetLayout set_layout = VK_NULL_HANDLE;
	VkPipelineLayout pipeline_layout = VK_NULL_HANDLE;
	VkDescriptorPool descriptor_pool = VK_NULL_HANDLE;
	VkDescriptorSet set = VK_NULL_HANDLE;
	VkShaderModule vert = VK_NULL_HANDLE, frag = VK_NULL_HANDLE;
	VkPipeline pipeline = VK_NULL_HANDLE;
	VkFormat pipeline_format = VK_FORMAT_UNDEFINED;

	// Décodeur et plans.
	pyrowave_decoder decoder = nullptr;
	Plane planes[3];
	VkFormat plane_format = VK_FORMAT_R8_UNORM;
	uint32_t width = 0, height = 0;
	bool chroma444 = false, hdr = false, full_range = true;
	float max_nits = 0, max_fall = 0, min_nits = 0;
	bool configured = false;
	std::string description;
};

namespace
{
void destroy_planes(kn_pw_session *s)
{
	VkDevice d = s->g->device;
	if (s->decoder)
	{
		pyrowave_decoder_destroy(s->decoder);
		s->decoder = nullptr;
	}
	for (auto &p : s->planes)
	{
		if (p.view)
			vkDestroyImageView(d, p.view, nullptr);
		if (p.image)
			vkDestroyImage(d, p.image, nullptr);
		if (p.memory)
			vkFreeMemory(d, p.memory, nullptr);
		p = {};
	}
}

void destroy_swapchain(kn_pw_session *s)
{
	VkDevice d = s->g->device;
	for (auto v : s->views)
		vkDestroyImageView(d, v, nullptr);
	for (auto sem : s->rendered)
		vkDestroySemaphore(d, sem, nullptr);
	s->views.clear();
	s->rendered.clear();
	s->images.clear();
	if (s->swapchain)
		vkDestroySwapchainKHR(d, s->swapchain, nullptr);
	s->swapchain = VK_NULL_HANDLE;
}

// Format et espace de couleur : HDR10 (A2B10G10R10, ST 2084) quand l'image
// l'est et que la surface l'accepte ; sinon un format 8 bits UNORM en sRGB
// non linéaire (la conversion écrit des valeurs déjà gamma).
bool pick_format(kn_pw_session *s, bool want_hdr, VkSurfaceFormatKHR &out)
{
	uint32_t n = 0;
	vkGetPhysicalDeviceSurfaceFormatsKHR(s->g->physical, s->surface, &n, nullptr);
	std::vector<VkSurfaceFormatKHR> formats(n);
	vkGetPhysicalDeviceSurfaceFormatsKHR(s->g->physical, s->surface, &n, formats.data());
	if (want_hdr && s->g->colorspace)
		for (auto &f : formats)
			if (f.colorSpace == VK_COLOR_SPACE_HDR10_ST2084_EXT &&
			    (f.format == VK_FORMAT_A2B10G10R10_UNORM_PACK32 || f.format == VK_FORMAT_A2R10G10B10_UNORM_PACK32))
			{
				out = f;
				return true;
			}
	for (VkFormat want : { VK_FORMAT_R8G8B8A8_UNORM, VK_FORMAT_B8G8R8A8_UNORM, VK_FORMAT_A2B10G10R10_UNORM_PACK32 })
		for (auto &f : formats)
			if (f.format == want && f.colorSpace == VK_COLOR_SPACE_SRGB_NONLINEAR_KHR)
			{
				out = f;
				return true;
			}
	return false;
}

bool create_pipeline(kn_pw_session *s)
{
	VkDevice d = s->g->device;
	if (s->pipeline && s->pipeline_format == s->format)
		return true;
	if (s->pipeline)
		vkDestroyPipeline(d, s->pipeline, nullptr);
	s->pipeline = VK_NULL_HANDLE;

	VkPipelineShaderStageCreateInfo stages[2] = {};
	stages[0] = { VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, nullptr, 0, VK_SHADER_STAGE_VERTEX_BIT, s->vert, "main" };
	stages[1] = { VK_STRUCTURE_TYPE_PIPELINE_SHADER_STAGE_CREATE_INFO, nullptr, 0, VK_SHADER_STAGE_FRAGMENT_BIT, s->frag, "main" };
	VkPipelineVertexInputStateCreateInfo vi = { VK_STRUCTURE_TYPE_PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO };
	VkPipelineInputAssemblyStateCreateInfo ia = { VK_STRUCTURE_TYPE_PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO };
	ia.topology = VK_PRIMITIVE_TOPOLOGY_TRIANGLE_LIST;
	VkPipelineViewportStateCreateInfo vp = { VK_STRUCTURE_TYPE_PIPELINE_VIEWPORT_STATE_CREATE_INFO };
	vp.viewportCount = 1;
	vp.scissorCount = 1;
	VkPipelineRasterizationStateCreateInfo rs = { VK_STRUCTURE_TYPE_PIPELINE_RASTERIZATION_STATE_CREATE_INFO };
	rs.polygonMode = VK_POLYGON_MODE_FILL;
	rs.cullMode = VK_CULL_MODE_NONE;
	rs.lineWidth = 1.0f;
	VkPipelineMultisampleStateCreateInfo ms = { VK_STRUCTURE_TYPE_PIPELINE_MULTISAMPLE_STATE_CREATE_INFO };
	ms.rasterizationSamples = VK_SAMPLE_COUNT_1_BIT;
	VkPipelineColorBlendAttachmentState att = {};
	att.colorWriteMask = 0xf;
	VkPipelineColorBlendStateCreateInfo cb = { VK_STRUCTURE_TYPE_PIPELINE_COLOR_BLEND_STATE_CREATE_INFO };
	cb.attachmentCount = 1;
	cb.pAttachments = &att;
	VkDynamicState dyn[] = { VK_DYNAMIC_STATE_VIEWPORT, VK_DYNAMIC_STATE_SCISSOR };
	VkPipelineDynamicStateCreateInfo ds = { VK_STRUCTURE_TYPE_PIPELINE_DYNAMIC_STATE_CREATE_INFO };
	ds.dynamicStateCount = 2;
	ds.pDynamicStates = dyn;
	VkPipelineRenderingCreateInfo rendering = { VK_STRUCTURE_TYPE_PIPELINE_RENDERING_CREATE_INFO };
	rendering.colorAttachmentCount = 1;
	rendering.pColorAttachmentFormats = &s->format;
	VkGraphicsPipelineCreateInfo info = { VK_STRUCTURE_TYPE_GRAPHICS_PIPELINE_CREATE_INFO, &rendering };
	info.stageCount = 2;
	info.pStages = stages;
	info.pVertexInputState = &vi;
	info.pInputAssemblyState = &ia;
	info.pViewportState = &vp;
	info.pRasterizationState = &rs;
	info.pMultisampleState = &ms;
	info.pColorBlendState = &cb;
	info.pDynamicState = &ds;
	info.layout = s->pipeline_layout;
	if (vkCreateGraphicsPipelines(d, VK_NULL_HANDLE, 1, &info, nullptr, &s->pipeline) != VK_SUCCESS)
		return false;
	s->pipeline_format = s->format;
	return true;
}

// (Re)crée la swapchain à la taille de la surface ; l'ancienne est passée en
// oldSwapchain puis détruite.
bool create_swapchain(kn_pw_session *s)
{
	Gpu *g = s->g;
	VkSurfaceCapabilitiesKHR caps;
	if (vkGetPhysicalDeviceSurfaceCapabilitiesKHR(g->physical, s->surface, &caps) != VK_SUCCESS)
		return false;
	VkSurfaceFormatKHR sf;
	if (!pick_format(s, s->hdr, sf))
	{
		LOGW("PyroWave : no usable surface format");
		return false;
	}
	uint32_t n = 0;
	vkGetPhysicalDeviceSurfacePresentModesKHR(g->physical, s->surface, &n, nullptr);
	std::vector<VkPresentModeKHR> modes(n);
	vkGetPhysicalDeviceSurfacePresentModesKHR(g->physical, s->surface, &n, modes.data());
	// Image montrée dès qu'elle est prête, la plus récente au prochain balayage
	// (comme MediaCodec) : MAILBOX, sinon FIFO.
	VkPresentModeKHR mode = std::find(modes.begin(), modes.end(), VK_PRESENT_MODE_MAILBOX_KHR) != modes.end()
	                            ? VK_PRESENT_MODE_MAILBOX_KHR
	                            : VK_PRESENT_MODE_FIFO_KHR;
	VkExtent2D extent = caps.currentExtent;
	if (extent.width == UINT32_MAX || extent.width == 0)
		extent = { uint32_t(std::max(1, ANativeWindow_getWidth(s->window))), uint32_t(std::max(1, ANativeWindow_getHeight(s->window))) };
	uint32_t count = std::max(caps.minImageCount + 1, 3u);
	if (caps.maxImageCount)
		count = std::min(count, caps.maxImageCount);
	VkCompositeAlphaFlagBitsKHR alpha = VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR;
	for (auto a : { VK_COMPOSITE_ALPHA_OPAQUE_BIT_KHR, VK_COMPOSITE_ALPHA_INHERIT_BIT_KHR, VK_COMPOSITE_ALPHA_PRE_MULTIPLIED_BIT_KHR })
		if (caps.supportedCompositeAlpha & a)
		{
			alpha = a;
			break;
		}

	VkSwapchainCreateInfoKHR info = { VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR };
	info.surface = s->surface;
	info.minImageCount = count;
	info.imageFormat = sf.format;
	info.imageColorSpace = sf.colorSpace;
	info.imageExtent = extent;
	info.imageArrayLayers = 1;
	info.imageUsage = VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT;
	info.imageSharingMode = VK_SHARING_MODE_EXCLUSIVE;
	// L'image reste droite : le compositeur tourne au besoin (comme MediaCodec).
	info.preTransform = (caps.supportedTransforms & VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR) ? VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR : caps.currentTransform;
	info.compositeAlpha = alpha;
	info.presentMode = mode;
	info.clipped = VK_TRUE;
	info.oldSwapchain = s->swapchain;
	VkSwapchainKHR swapchain = VK_NULL_HANDLE;
	VkResult r = vkCreateSwapchainKHR(g->device, &info, nullptr, &swapchain);
	destroy_swapchain(s);
	if (r != VK_SUCCESS)
	{
		LOGW("PyroWave : vkCreateSwapchainKHR %d", int(r));
		return false;
	}
	s->swapchain = swapchain;
	s->format = sf.format;
	s->colorspace = sf.colorSpace;
	s->extent = extent;
	s->hdr_output = sf.colorSpace == VK_COLOR_SPACE_HDR10_ST2084_EXT;
	vkGetSwapchainImagesKHR(g->device, swapchain, &n, nullptr);
	s->images.resize(n);
	vkGetSwapchainImagesKHR(g->device, swapchain, &n, s->images.data());
	for (auto img : s->images)
	{
		VkImageViewCreateInfo vi = { VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO };
		vi.image = img;
		vi.viewType = VK_IMAGE_VIEW_TYPE_2D;
		vi.format = sf.format;
		vi.subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 };
		VkImageView v = VK_NULL_HANDLE;
		vkCreateImageView(g->device, &vi, nullptr, &v);
		s->views.push_back(v);
		VkSemaphoreCreateInfo si = { VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO };
		VkSemaphore sem = VK_NULL_HANDLE;
		vkCreateSemaphore(g->device, &si, nullptr, &sem);
		s->rendered.push_back(sem);
	}
	if (s->hdr_output && g->hdr_metadata && s->max_nits > 0)
	{
		VkHdrMetadataEXT md = { VK_STRUCTURE_TYPE_HDR_METADATA_EXT };
		// Primaires BT.2020, blanc D65.
		md.displayPrimaryRed = { 0.708f, 0.292f };
		md.displayPrimaryGreen = { 0.170f, 0.797f };
		md.displayPrimaryBlue = { 0.131f, 0.046f };
		md.whitePoint = { 0.3127f, 0.3290f };
		md.maxLuminance = s->max_nits;
		md.minLuminance = s->min_nits;
		md.maxContentLightLevel = s->max_nits;
		md.maxFrameAverageLightLevel = s->max_fall > 0 ? s->max_fall : s->max_nits;
		vkSetHdrMetadataEXT(g->device, 1, &s->swapchain, &md);
	}
	if (!create_pipeline(s))
	{
		LOGW("PyroWave : pipeline creation failed");
		return false;
	}
	LOGI("PyroWave : swapchain %ux%u, %u images, format %d, %s, %s", extent.width, extent.height, n, int(sf.format),
	     s->hdr_output ? "HDR10 ST2084" : "sRGB", mode == VK_PRESENT_MODE_MAILBOX_KHR ? "mailbox" : "fifo");
	return true;
}

bool create_plane(kn_pw_session *s, Plane &p, uint32_t w, uint32_t h)
{
	VkDevice d = s->g->device;
	VkImageCreateInfo ii = { VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO };
	ii.imageType = VK_IMAGE_TYPE_2D;
	ii.format = s->plane_format;
	ii.extent = { w, h, 1 };
	ii.mipLevels = 1;
	ii.arrayLayers = 1;
	ii.samples = VK_SAMPLE_COUNT_1_BIT;
	ii.tiling = VK_IMAGE_TILING_OPTIMAL;
	ii.usage = VK_IMAGE_USAGE_SAMPLED_BIT | (s->g->fragment ? VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT : VK_IMAGE_USAGE_STORAGE_BIT);
	ii.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
	ii.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;
	if (vkCreateImage(d, &ii, nullptr, &p.image) != VK_SUCCESS)
		return false;
	VkMemoryRequirements req;
	vkGetImageMemoryRequirements(d, p.image, &req);
	VkMemoryAllocateInfo ai = { VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO };
	ai.allocationSize = req.size;
	ai.memoryTypeIndex = memory_type(s->g, req.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
	if (vkAllocateMemory(d, &ai, nullptr, &p.memory) != VK_SUCCESS || vkBindImageMemory(d, p.image, p.memory, 0) != VK_SUCCESS)
		return false;
	VkImageViewCreateInfo vi = { VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO };
	vi.image = p.image;
	vi.viewType = VK_IMAGE_VIEW_TYPE_2D;
	vi.format = s->plane_format;
	vi.subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 };
	if (vkCreateImageView(d, &vi, nullptr, &p.view) != VK_SUCCESS)
		return false;
	p.width = w;
	p.height = h;
	return true;
}

void image_barrier(VkCommandBuffer cmd, VkImage image, VkImageLayout from, VkImageLayout to, VkPipelineStageFlags2 src_stage,
                   VkAccessFlags2 src_access, VkPipelineStageFlags2 dst_stage, VkAccessFlags2 dst_access)
{
	VkImageMemoryBarrier2 b = { VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER_2 };
	b.srcStageMask = src_stage;
	b.srcAccessMask = src_access;
	b.dstStageMask = dst_stage;
	b.dstAccessMask = dst_access;
	b.oldLayout = from;
	b.newLayout = to;
	b.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
	b.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
	b.image = image;
	b.subresourceRange = { VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1 };
	VkDependencyInfo dep = { VK_STRUCTURE_TYPE_DEPENDENCY_INFO };
	dep.imageMemoryBarrierCount = 1;
	dep.pImageMemoryBarriers = &b;
	vkCmdPipelineBarrier2(cmd, &dep);
}

PushConstants push_constants(const kn_pw_session *s)
{
	PushConstants pc = {};
	// Décalage et échelle de Y puis de la chroma. 10 bits (HDR) : convention
	// P010, un code c sur 10 bits vaut c · 64 / 65 535 une fois normalisé.
	if (s->full_range)
	{
		pc.range[0] = 0.0f;
		pc.range[1] = 1.0f;
		pc.range[2] = s->hdr ? 512.0f * 64.0f / 65535.0f : 128.0f / 255.0f;
		pc.range[3] = 1.0f;
	}
	else if (s->hdr)
	{
		pc.range[0] = 64.0f * 64.0f / 65535.0f;
		pc.range[1] = 65535.0f / (876.0f * 64.0f);
		pc.range[2] = 512.0f * 64.0f / 65535.0f;
		pc.range[3] = 65535.0f / (896.0f * 64.0f);
	}
	else
	{
		pc.range[0] = 16.0f / 255.0f;
		pc.range[1] = 255.0f / 219.0f;
		pc.range[2] = 128.0f / 255.0f;
		pc.range[3] = 255.0f / 224.0f;
	}
	// BT.709 en SDR, BT.2020 (non constant) en HDR10.
	float kr = s->hdr ? 0.2627f : 0.2126f, kb = s->hdr ? 0.0593f : 0.0722f, kg = 1.0f - kr - kb;
	pc.coef[0] = 2.0f - 2.0f * kr;
	pc.coef[1] = kb * (2.0f - 2.0f * kb) / kg;
	pc.coef[2] = kr * (2.0f - 2.0f * kr) / kg;
	pc.coef[3] = 2.0f - 2.0f * kb;
	pc.mode = !s->hdr ? 0 : s->hdr_output ? 1 : 2;
	pc.sdr_white = 203.0f;
	return pc;
}

void write_descriptors(kn_pw_session *s)
{
	VkDescriptorImageInfo ii[3];
	VkWriteDescriptorSet w[3];
	for (int i = 0; i < 3; i++)
	{
		ii[i] = { s->sampler, s->planes[i].view, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL };
		w[i] = { VK_STRUCTURE_TYPE_WRITE_DESCRIPTOR_SET };
		w[i].dstSet = s->set;
		w[i].dstBinding = uint32_t(i);
		w[i].descriptorCount = 1;
		w[i].descriptorType = VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER;
		w[i].pImageInfo = &ii[i];
	}
	vkUpdateDescriptorSets(s->g->device, 3, w, 0, nullptr);
}

void describe(kn_pw_session *s)
{
	char b[256];
	snprintf(b, sizeof(b), "PyroWave (Vulkan, %s, %s%s)", s->g->name.c_str(), s->g->fragment ? "fragment" : "compute",
	         s->hdr ? (s->hdr_output ? ", HDR10" : ", HDR10 → SDR") : "");
	s->description = b;
}

bool create_static(kn_pw_session *s)
{
	VkDevice d = s->g->device;
	VkCommandPoolCreateInfo pi = { VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO };
	pi.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
	pi.queueFamilyIndex = s->g->family;
	if (vkCreateCommandPool(d, &pi, nullptr, &s->pool) != VK_SUCCESS)
		return false;
	VkCommandBufferAllocateInfo ai = { VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO };
	ai.commandPool = s->pool;
	ai.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
	ai.commandBufferCount = 1;
	if (vkAllocateCommandBuffers(d, &ai, &s->cmd) != VK_SUCCESS)
		return false;
	VkFenceCreateInfo fi = { VK_STRUCTURE_TYPE_FENCE_CREATE_INFO };
	VkSemaphoreCreateInfo si = { VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO };
	if (vkCreateFence(d, &fi, nullptr, &s->fence) != VK_SUCCESS || vkCreateSemaphore(d, &si, nullptr, &s->acquired) != VK_SUCCESS)
		return false;

	VkDescriptorSetLayoutBinding b[3];
	for (uint32_t i = 0; i < 3; i++)
		b[i] = { i, VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 1, VK_SHADER_STAGE_FRAGMENT_BIT, nullptr };
	VkDescriptorSetLayoutCreateInfo li = { VK_STRUCTURE_TYPE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO };
	li.bindingCount = 3;
	li.pBindings = b;
	if (vkCreateDescriptorSetLayout(d, &li, nullptr, &s->set_layout) != VK_SUCCESS)
		return false;
	VkPushConstantRange range = { VK_SHADER_STAGE_FRAGMENT_BIT, 0, sizeof(PushConstants) };
	VkPipelineLayoutCreateInfo pl = { VK_STRUCTURE_TYPE_PIPELINE_LAYOUT_CREATE_INFO };
	pl.setLayoutCount = 1;
	pl.pSetLayouts = &s->set_layout;
	pl.pushConstantRangeCount = 1;
	pl.pPushConstantRanges = &range;
	if (vkCreatePipelineLayout(d, &pl, nullptr, &s->pipeline_layout) != VK_SUCCESS)
		return false;
	VkDescriptorPoolSize size = { VK_DESCRIPTOR_TYPE_COMBINED_IMAGE_SAMPLER, 3 };
	VkDescriptorPoolCreateInfo dp = { VK_STRUCTURE_TYPE_DESCRIPTOR_POOL_CREATE_INFO };
	dp.maxSets = 1;
	dp.poolSizeCount = 1;
	dp.pPoolSizes = &size;
	if (vkCreateDescriptorPool(d, &dp, nullptr, &s->descriptor_pool) != VK_SUCCESS)
		return false;
	VkDescriptorSetAllocateInfo da = { VK_STRUCTURE_TYPE_DESCRIPTOR_SET_ALLOCATE_INFO };
	da.descriptorPool = s->descriptor_pool;
	da.descriptorSetCount = 1;
	da.pSetLayouts = &s->set_layout;
	if (vkAllocateDescriptorSets(d, &da, &s->set) != VK_SUCCESS)
		return false;
	VkShaderModuleCreateInfo sm = { VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO };
	sm.codeSize = sizeof(present_vert);
	sm.pCode = present_vert;
	if (vkCreateShaderModule(d, &sm, nullptr, &s->vert) != VK_SUCCESS)
		return false;
	sm.codeSize = sizeof(present_frag);
	sm.pCode = present_frag;
	return vkCreateShaderModule(d, &sm, nullptr, &s->frag) == VK_SUCCESS;
}

void destroy_sampler(kn_pw_session *s)
{
	if (s->sampler)
		vkDestroySampler(s->g->device, s->sampler, nullptr);
	s->sampler = VK_NULL_HANDLE;
}

bool create_sampler(kn_pw_session *s)
{
	destroy_sampler(s);
	VkSamplerCreateInfo si = { VK_STRUCTURE_TYPE_SAMPLER_CREATE_INFO };
	VkFilter f = linear_filter(s->g, s->plane_format) ? VK_FILTER_LINEAR : VK_FILTER_NEAREST;
	si.magFilter = f;
	si.minFilter = f;
	si.mipmapMode = VK_SAMPLER_MIPMAP_MODE_NEAREST;
	si.addressModeU = si.addressModeV = si.addressModeW = VK_SAMPLER_ADDRESS_MODE_CLAMP_TO_EDGE;
	si.maxLod = 0.0f;
	return vkCreateSampler(s->g->device, &si, nullptr, &s->sampler) == VK_SUCCESS;
}

// Décodeur et plans pour ce format ; la swapchain si le HDR change.
bool configure_locked(kn_pw_session *s)
{
	Gpu *g = s->g;
	vkDeviceWaitIdle(g->device);
	destroy_planes(s);
	s->configured = false;
	s->plane_format = s->hdr && plane_format_ok(g, VK_FORMAT_R16_UNORM) ? VK_FORMAT_R16_UNORM : VK_FORMAT_R8_UNORM;
	if (!plane_format_ok(g, s->plane_format))
	{
		LOGW("PyroWave : plane format unsupported");
		return false;
	}
	pyrowave_decoder_create_info di = {};
	di.device = g->pyro;
	di.width = int(s->width);
	di.height = int(s->height);
	di.chroma = s->chroma444 ? PYROWAVE_CHROMA_SUBSAMPLING_444 : PYROWAVE_CHROMA_SUBSAMPLING_420;
	di.fragment_path = g->fragment;
	pyrowave_result r = pyrowave_decoder_create(&di, &s->decoder);
	if (r != PYROWAVE_SUCCESS)
	{
		LOGW("PyroWave : decoder %ux%u failed (%d)", s->width, s->height, int(r));
		s->decoder = nullptr;
		return false;
	}
	uint32_t cw = s->chroma444 ? s->width : s->width / 2, ch = s->chroma444 ? s->height : s->height / 2;
	if (!create_plane(s, s->planes[0], s->width, s->height) || !create_plane(s, s->planes[1], cw, ch) ||
	    !create_plane(s, s->planes[2], cw, ch) || !create_sampler(s))
	{
		LOGW("PyroWave : planes failed");
		destroy_planes(s);
		return false;
	}
	write_descriptors(s);
	bool want_hdr_output = s->hdr && g->colorspace;
	if (!s->swapchain || want_hdr_output != s->hdr_output)
		if (!create_swapchain(s))
			return false;
	describe(s);
	s->configured = true;
	LOGI("PyroWave : decoder %ux%u %s, %s %s, planes %s", s->width, s->height, s->chroma444 ? "4:4:4" : "4:2:0",
	     s->hdr ? "HDR10" : "SDR", s->full_range ? "full" : "limited", s->plane_format == VK_FORMAT_R16_UNORM ? "R16" : "R8");
	return true;
}

int frame_locked(kn_pw_session *s, const uint8_t *data, size_t size)
{
	Gpu *g = s->g;
	if (!s->configured)
		return KN_PW_INCOMPLETE;
	pyrowave_decoder_clear(s->decoder);
	if (pyrowave_decoder_push_packet(s->decoder, data, size) != PYROWAVE_SUCCESS)
		return KN_PW_INCOMPLETE;
	// Image complète, ou presque (bandes basses là, 90 % des blocs) : sinon sautée.
	if (!pyrowave_decoder_decode_is_ready(s->decoder, false) && !pyrowave_decoder_decode_is_ready(s->decoder, true))
		return KN_PW_INCOMPLETE;

	// Une recréation ratée (surface en transition : fenêtre redimensionnée,
	// réduite, passée sur un autre écran) laisse la swapchain nulle :
	// vkAcquireNextImageKHR sur VK_NULL_HANDLE plante dans le pilote (AND-02).
	// On la refait, sinon la session PyroWave est à refaire.
	if (s->swapchain == VK_NULL_HANDLE && !create_swapchain(s))
		return KN_PW_LOST;

	uint32_t index = 0;
	VkResult r = VK_SUCCESS;
	for (int attempt = 0; attempt < 2; attempt++)
	{
		r = vkAcquireNextImageKHR(g->device, s->swapchain, 100'000'000ull, s->acquired, VK_NULL_HANDLE, &index);
		if (r == VK_ERROR_OUT_OF_DATE_KHR)
		{
			vkDeviceWaitIdle(g->device);
			if (!create_swapchain(s))
				return KN_PW_LOST;
			continue;
		}
		break;
	}
	if (r == VK_TIMEOUT || r == VK_NOT_READY)
		return KN_PW_BUSY;
	if (r != VK_SUCCESS && r != VK_SUBOPTIMAL_KHR)
	{
		LOGW("PyroWave : vkAcquireNextImageKHR %d", int(r));
		return KN_PW_LOST;
	}

	VkCommandBuffer cmd = s->cmd;
	vkResetCommandBuffer(cmd, 0);
	VkCommandBufferBeginInfo bi = { VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO };
	bi.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
	vkBeginCommandBuffer(cmd, &bi);

	// Plans : contenu ancien jeté (l'image décodée les couvre entièrement).
	VkPipelineStageFlags2 dec_stage = g->fragment ? VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT : VK_PIPELINE_STAGE_2_COMPUTE_SHADER_BIT;
	VkAccessFlags2 dec_access = g->fragment ? VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT : VK_ACCESS_2_SHADER_STORAGE_WRITE_BIT;
	VkImageLayout dec_layout = g->fragment ? VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL : VK_IMAGE_LAYOUT_GENERAL;
	for (auto &p : s->planes)
		image_barrier(cmd, p.image, VK_IMAGE_LAYOUT_UNDEFINED, dec_layout, VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT, 0, dec_stage, dec_access);

	pyrowave_gpu_buffers buffers = {};
	for (int i = 0; i < 3; i++)
	{
		auto &v = buffers.planes[i];
		v.image = s->planes[i].image;
		v.width = s->planes[0].width;
		v.height = s->planes[0].height;
		v.image_format = s->plane_format;
		v.view_format = s->plane_format;
		v.aspect = VK_IMAGE_ASPECT_COLOR_BIT;
		v.swizzle = VK_COMPONENT_SWIZZLE_IDENTITY;
		v.layout = dec_layout;
	}
	pyrowave_device_set_command_buffer(g->pyro, cmd);
	pyrowave_result dr = pyrowave_decoder_decode_gpu_buffer(s->decoder, nullptr, nullptr, &buffers);
	pyrowave_device_set_command_buffer(g->pyro, VK_NULL_HANDLE);
	if (dr != PYROWAVE_SUCCESS)
		LOGW("PyroWave : decode failed (%d)", int(dr));

	for (auto &p : s->planes)
		image_barrier(cmd, p.image, dec_layout, VK_IMAGE_LAYOUT_SHADER_READ_ONLY_OPTIMAL, dec_stage, dec_access,
		              VK_PIPELINE_STAGE_2_FRAGMENT_SHADER_BIT, VK_ACCESS_2_SHADER_SAMPLED_READ_BIT);
	VkImage target = s->images[index];
	image_barrier(cmd, target, VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL, VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT, 0,
	              VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT, VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT);

	VkRenderingAttachmentInfo color = { VK_STRUCTURE_TYPE_RENDERING_ATTACHMENT_INFO };
	color.imageView = s->views[index];
	color.imageLayout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL;
	color.loadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE;
	color.storeOp = VK_ATTACHMENT_STORE_OP_STORE;
	VkRenderingInfo ri = { VK_STRUCTURE_TYPE_RENDERING_INFO };
	ri.renderArea = { { 0, 0 }, s->extent };
	ri.layerCount = 1;
	ri.colorAttachmentCount = 1;
	ri.pColorAttachments = &color;
	vkCmdBeginRendering(cmd, &ri);
	VkViewport viewport = { 0, 0, float(s->extent.width), float(s->extent.height), 0, 1 };
	VkRect2D scissor = { { 0, 0 }, s->extent };
	vkCmdSetViewport(cmd, 0, 1, &viewport);
	vkCmdSetScissor(cmd, 0, 1, &scissor);
	vkCmdBindPipeline(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, s->pipeline);
	vkCmdBindDescriptorSets(cmd, VK_PIPELINE_BIND_POINT_GRAPHICS, s->pipeline_layout, 0, 1, &s->set, 0, nullptr);
	PushConstants pc = push_constants(s);
	vkCmdPushConstants(cmd, s->pipeline_layout, VK_SHADER_STAGE_FRAGMENT_BIT, 0, sizeof(pc), &pc);
	vkCmdDraw(cmd, 3, 1, 0, 0);
	vkCmdEndRendering(cmd);
	image_barrier(cmd, target, VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT,
	              VK_ACCESS_2_COLOR_ATTACHMENT_WRITE_BIT, VK_PIPELINE_STAGE_2_NONE, 0);
	vkEndCommandBuffer(cmd);

	VkSemaphoreSubmitInfo wait = { VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO };
	wait.semaphore = s->acquired;
	wait.stageMask = VK_PIPELINE_STAGE_2_COLOR_ATTACHMENT_OUTPUT_BIT;
	VkSemaphoreSubmitInfo signal = { VK_STRUCTURE_TYPE_SEMAPHORE_SUBMIT_INFO };
	signal.semaphore = s->rendered[index];
	signal.stageMask = VK_PIPELINE_STAGE_2_ALL_COMMANDS_BIT;
	VkCommandBufferSubmitInfo cbi = { VK_STRUCTURE_TYPE_COMMAND_BUFFER_SUBMIT_INFO };
	cbi.commandBuffer = cmd;
	VkSubmitInfo2 submit = { VK_STRUCTURE_TYPE_SUBMIT_INFO_2 };
	submit.waitSemaphoreInfoCount = 1;
	submit.pWaitSemaphoreInfos = &wait;
	submit.commandBufferInfoCount = 1;
	submit.pCommandBufferInfos = &cbi;
	submit.signalSemaphoreInfoCount = 1;
	submit.pSignalSemaphoreInfos = &signal;
	r = vkQueueSubmit2(g->queue, 1, &submit, s->fence);
	if (r != VK_SUCCESS)
	{
		LOGW("PyroWave : vkQueueSubmit2 %d", int(r));
		return KN_PW_LOST;
	}
	VkPresentInfoKHR pi = { VK_STRUCTURE_TYPE_PRESENT_INFO_KHR };
	pi.waitSemaphoreCount = 1;
	pi.pWaitSemaphores = &s->rendered[index];
	pi.swapchainCount = 1;
	pi.pSwapchains = &s->swapchain;
	pi.pImageIndices = &index;
	VkResult pr = vkQueuePresentKHR(g->queue, &pi);
	// GPU terminé avant de rendre la main : l'instant du décodage pour les
	// statistiques, et les tampons de PyroWave libres pour l'image suivante.
	r = vkWaitForFences(g->device, 1, &s->fence, VK_TRUE, 1'000'000'000ull);
	if (r != VK_SUCCESS)
	{
		// Clôture encore en vol : la remettre à zéro est interdit (et plante
		// certains pilotes, AND-03). On attend que le GPU ait fini, puis on la
		// remet à zéro, et la session est à refaire.
		LOGW("PyroWave : GPU did not finish (%d)", int(r));
		vkDeviceWaitIdle(g->device);
		vkResetFences(g->device, 1, &s->fence);
		return KN_PW_LOST;
	}
	vkResetFences(g->device, 1, &s->fence);
	if (pr == VK_ERROR_OUT_OF_DATE_KHR || pr == VK_SUBOPTIMAL_KHR)
	{
		vkDeviceWaitIdle(g->device);
		if (!create_swapchain(s))
			return KN_PW_LOST;
	}
	else if (pr != VK_SUCCESS)
	{
		LOGW("PyroWave : vkQueuePresentKHR %d", int(pr));
		return KN_PW_LOST;
	}
	return dr == PYROWAVE_SUCCESS ? KN_PW_SHOWN : KN_PW_INCOMPLETE;
}

void close_locked(kn_pw_session *s)
{
	if (!s->g)
	{
		delete s;
		return;
	}
	VkDevice d = s->g->device;
	vkDeviceWaitIdle(d);
	destroy_planes(s);
	destroy_swapchain(s);
	destroy_sampler(s);
	if (s->pipeline)
		vkDestroyPipeline(d, s->pipeline, nullptr);
	if (s->vert)
		vkDestroyShaderModule(d, s->vert, nullptr);
	if (s->frag)
		vkDestroyShaderModule(d, s->frag, nullptr);
	if (s->descriptor_pool)
		vkDestroyDescriptorPool(d, s->descriptor_pool, nullptr);
	if (s->pipeline_layout)
		vkDestroyPipelineLayout(d, s->pipeline_layout, nullptr);
	if (s->set_layout)
		vkDestroyDescriptorSetLayout(d, s->set_layout, nullptr);
	if (s->acquired)
		vkDestroySemaphore(d, s->acquired, nullptr);
	if (s->fence)
		vkDestroyFence(d, s->fence, nullptr);
	if (s->pool)
		vkDestroyCommandPool(d, s->pool, nullptr);
	if (s->surface)
		vkDestroySurfaceKHR(s->g->instance, s->surface, nullptr);
	if (s->window)
		ANativeWindow_release(s->window);
	delete s;
}
} // namespace

extern "C" {
int kn_pw_probe(char *report, size_t report_size)
{
	std::lock_guard<std::mutex> holder(lock);
	Gpu *g = gpu();
	if (!g)
	{
		put(report, report_size, "%s", gpu_error.c_str());
		return 0;
	}
	if (!g->swapchain)
	{
		put(report, report_size, "%s: no Android swapchain", g->name.c_str());
		return 0;
	}
	pyrowave_decoder_create_info di = {};
	di.device = g->pyro;
	di.width = 128;
	di.height = 128;
	di.chroma = PYROWAVE_CHROMA_SUBSAMPLING_420;
	di.fragment_path = g->fragment;
	pyrowave_decoder dec = nullptr;
	if (pyrowave_decoder_create(&di, &dec) != PYROWAVE_SUCCESS)
	{
		put(report, report_size, "%s: no decoder", g->name.c_str());
		return 0;
	}
	bool ok = pyrowave_decoder_push_packet(dec, probe_frame, sizeof(probe_frame)) == PYROWAVE_SUCCESS &&
	          pyrowave_decoder_decode_is_ready(dec, false);
	std::vector<uint8_t> y(128 * 128), cb(64 * 64), cr(64 * 64);
	if (ok)
	{
		pyrowave_cpu_buffer buf = {};
		buf.data[0] = y.data();
		buf.data[1] = cb.data();
		buf.data[2] = cr.data();
		buf.row_stride_in_bytes[0] = 128;
		buf.row_stride_in_bytes[1] = buf.row_stride_in_bytes[2] = 64;
		buf.plane_size_in_bytes[0] = y.size();
		buf.plane_size_in_bytes[1] = buf.plane_size_in_bytes[2] = cb.size();
		buf.width = 128;
		buf.height = 128;
		buf.format = PYROWAVE_CPU_BUFFER_FORMAT_YUV420P;
		ok = pyrowave_decoder_decode_cpu_buffer_synchronous(dec, &buf) == PYROWAVE_SUCCESS;
	}
	pyrowave_decoder_destroy(dec);
	if (!ok)
	{
		put(report, report_size, "%s: test frame not decoded", g->name.c_str());
		return 0;
	}
	// Rampe de Y de 16 (à gauche) à 235 (à droite), chroma à 128 : à 4 près.
	auto near = [](int v, double want) { return std::fabs(double(v) - want) <= 4.0; };
	for (int x : { 0, 32, 64, 96, 127 })
		if (!near(y[64 * 128 + x], 16.0 + double(x) * 219.0 / 127.0))
		{
			put(report, report_size, "%s: wrong test frame (Y[%d] = %d)", g->name.c_str(), x, int(y[64 * 128 + x]));
			return 0;
		}
	for (int x : { 0, 31, 63 })
		if (!near(cb[32 * 64 + x], 128) || !near(cr[32 * 64 + x], 128))
		{
			put(report, report_size, "%s: wrong test frame (chroma)", g->name.c_str());
			return 0;
		}
	put(report, report_size, "%s, %s", g->name.c_str(), g->fragment ? "fragment" : "compute");
	return 1;
}

kn_pw_session *kn_pw_open(ANativeWindow *window, char *error, size_t error_size)
{
	std::lock_guard<std::mutex> holder(lock);
	Gpu *g = gpu();
	if (!g || !g->swapchain)
	{
		put(error, error_size, "%s", g ? "no swapchain" : gpu_error.c_str());
		return nullptr;
	}
	auto *s = new kn_pw_session();
	s->g = g;
	ANativeWindow_acquire(window);
	s->window = window;
	VkAndroidSurfaceCreateInfoKHR si = { VK_STRUCTURE_TYPE_ANDROID_SURFACE_CREATE_INFO_KHR };
	si.window = window;
	VkResult r = g->create_surface(g->instance, &si, nullptr, &s->surface);
	if (r != VK_SUCCESS || !create_static(s))
	{
		put(error, error_size, "surface or resources failed (%d)", int(r));
		close_locked(s);
		return nullptr;
	}
	return s;
}

int kn_pw_configure(kn_pw_session *s, uint32_t width, uint32_t height, int chroma444, int hdr, int full_range, float max_nits,
                    float max_fall, float min_nits)
{
	std::lock_guard<std::mutex> holder(lock);
	if (!s)
		return 0;
	if (s->configured && s->width == width && s->height == height && s->chroma444 == bool(chroma444) && s->hdr == bool(hdr) &&
	    s->full_range == bool(full_range))
		return 1;
	s->width = width;
	s->height = height;
	s->chroma444 = chroma444 != 0;
	s->hdr = hdr != 0;
	s->full_range = full_range != 0;
	s->max_nits = max_nits;
	s->max_fall = max_fall;
	s->min_nits = min_nits;
	return configure_locked(s) ? 1 : 0;
}

int kn_pw_frame(kn_pw_session *s, const uint8_t *data, size_t size)
{
	std::lock_guard<std::mutex> holder(lock);
	if (!s)
		return KN_PW_LOST;
	return frame_locked(s, data, size);
}

const char *kn_pw_describe(kn_pw_session *s)
{
	return s ? s->description.c_str() : "";
}

int kn_pw_hdr_output(kn_pw_session *s)
{
	return s && s->hdr_output ? 1 : 0;
}

void kn_pw_close(kn_pw_session *s)
{
	std::lock_guard<std::mutex> holder(lock);
	if (s)
		close_locked(s);
}
}
