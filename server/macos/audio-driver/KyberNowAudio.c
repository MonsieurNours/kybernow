// KyberNow : sortie son virtuelle de la Station Mac (docs/station-macos.md, jalon 4).
//
// Module Core Audio (AudioServerPlugIn) chargé par coreaudiod : un appareil
// « KyberNow » avec une sortie de 2, 6 ou 8 canaux (48 kHz, flottants) qui ne
// joue rien sur le Mac, et une entrée de retour qui rend ce qu'on lui a joué.
// Pendant une session, Admin y bascule la sortie du Mac au nombre de canaux de
// l'appareil (agent `kybernow-display audio-route`) ; Kyber capture le son.
//
// Objets : le module (kAudioObjectPlugInObject), l'appareil, son flux
// d'entrée (retour) et son flux de sortie. Pas de commandes (volume) : le son
// de la session suit le volume de l'appareil qui le reçoit.
//
// Horloge : un tour d'anneau toutes les kRingFrames images, d'après
// mach_absolute_time, comme le modèle de pilote d'Apple. Le flux de sortie
// écrit dans l'anneau à son heure ; l'entrée lit à la sienne (passée) et vide
// ce qu'elle a lu : sans rien de joué, le retour est silencieux.

#include <CoreAudio/AudioServerPlugIn.h>
#include <CoreFoundation/CoreFoundation.h>
#include <dispatch/dispatch.h>
#include <mach/mach_time.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <string.h>

#define kDeviceUID "fr.leslignesclaires.kybernow.audio.device"
#define kModelUID "fr.leslignesclaires.kybernow.audio.model"
#define kDeviceName "KyberNow"
#define kManufacturer "KyberNow"

enum {
    kObjectPlugIn = kAudioObjectPlugInObject,
    kObjectDevice = 2,
    kObjectStreamInput = 3,
    kObjectStreamOutput = 4,
};

/// Images par tour d'anneau (et période de l'horodatage zéro).
#define kRingFrames 16384
#define kMaxChannels 8
static const Float64 kSampleRates[] = {44100.0, 48000.0};
#define kSampleRateCount (sizeof(kSampleRates) / sizeof(kSampleRates[0]))
static const UInt32 kChannelCounts[] = {2, 6, 8};
#define kChannelCountCount (sizeof(kChannelCounts) / sizeof(kChannelCounts[0]))
static const UInt32 kSafetyOffset = 0;
static const UInt32 kLatency = 0;

// ----------------------------------------------------------------------------
// État

static AudioServerPlugInHostRef gHost = NULL;
static pthread_mutex_t gStateMutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_mutex_t gIOMutex = PTHREAD_MUTEX_INITIALIZER;
static UInt32 gRefCount = 0;

static Float64 gSampleRate = 48000.0;
static UInt32 gChannels = 2;
static bool gInputActive = true;
static bool gOutputActive = true;

static UInt32 gIORunning = 0;
static Float64 gHostTicksPerFrame = 0.0;
static UInt64 gAnchorHostTime = 0;
static UInt64 gNumberTimeStamps = 0;

static Float32 gRing[kRingFrames * kMaxChannels];

/// Changement demandé par SetPropertyData, appliqué par PerformDeviceConfigurationChange.
static Float64 gPendingSampleRate = 0.0;
static UInt32 gPendingChannels = 0;

static void UpdateTicksPerFrame(void) {
    struct mach_timebase_info tb;
    mach_timebase_info(&tb);
    Float64 ticksPerSecond = 1e9 * (Float64)tb.denom / (Float64)tb.numer;
    gHostTicksPerFrame = ticksPerSecond / gSampleRate;
}

static AudioStreamBasicDescription FormatFor(Float64 rate, UInt32 channels) {
    AudioStreamBasicDescription f;
    memset(&f, 0, sizeof(f));
    f.mSampleRate = rate;
    f.mFormatID = kAudioFormatLinearPCM;
    f.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
    f.mBytesPerPacket = 4 * channels;
    f.mFramesPerPacket = 1;
    f.mBytesPerFrame = 4 * channels;
    f.mChannelsPerFrame = channels;
    f.mBitsPerChannel = 32;
    return f;
}

static bool SupportedChannels(UInt32 c) {
    for (UInt32 i = 0; i < kChannelCountCount; i++)
        if (kChannelCounts[i] == c) return true;
    return false;
}

static bool SupportedRate(Float64 r) {
    for (UInt32 i = 0; i < kSampleRateCount; i++)
        if (kSampleRates[i] == r) return true;
    return false;
}

// ----------------------------------------------------------------------------
// Interface COM

static HRESULT KN_QueryInterface(void *inDriver, REFIID inUUID, LPVOID *outInterface);
static ULONG KN_AddRef(void *inDriver);
static ULONG KN_Release(void *inDriver);
static OSStatus KN_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost);
static OSStatus KN_CreateDevice(AudioServerPlugInDriverRef, CFDictionaryRef, const AudioServerPlugInClientInfo *, AudioObjectID *);
static OSStatus KN_DestroyDevice(AudioServerPlugInDriverRef, AudioObjectID);
static OSStatus KN_AddDeviceClient(AudioServerPlugInDriverRef, AudioObjectID, const AudioServerPlugInClientInfo *);
static OSStatus KN_RemoveDeviceClient(AudioServerPlugInDriverRef, AudioObjectID, const AudioServerPlugInClientInfo *);
static OSStatus KN_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef, AudioObjectID, UInt64, void *);
static OSStatus KN_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef, AudioObjectID, UInt64, void *);
static Boolean KN_HasProperty(AudioServerPlugInDriverRef, AudioObjectID, pid_t, const AudioObjectPropertyAddress *);
static OSStatus KN_IsPropertySettable(AudioServerPlugInDriverRef, AudioObjectID, pid_t, const AudioObjectPropertyAddress *, Boolean *);
static OSStatus KN_GetPropertyDataSize(AudioServerPlugInDriverRef, AudioObjectID, pid_t, const AudioObjectPropertyAddress *, UInt32, const void *, UInt32 *);
static OSStatus KN_GetPropertyData(AudioServerPlugInDriverRef, AudioObjectID, pid_t, const AudioObjectPropertyAddress *, UInt32, const void *, UInt32, UInt32 *, void *);
static OSStatus KN_SetPropertyData(AudioServerPlugInDriverRef, AudioObjectID, pid_t, const AudioObjectPropertyAddress *, UInt32, const void *, UInt32, const void *);
static OSStatus KN_StartIO(AudioServerPlugInDriverRef, AudioObjectID, UInt32);
static OSStatus KN_StopIO(AudioServerPlugInDriverRef, AudioObjectID, UInt32);
static OSStatus KN_GetZeroTimeStamp(AudioServerPlugInDriverRef, AudioObjectID, UInt32, Float64 *, UInt64 *, UInt64 *);
static OSStatus KN_WillDoIOOperation(AudioServerPlugInDriverRef, AudioObjectID, UInt32, UInt32, Boolean *, Boolean *);
static OSStatus KN_BeginIOOperation(AudioServerPlugInDriverRef, AudioObjectID, UInt32, UInt32, UInt32, const AudioServerPlugInIOCycleInfo *);
static OSStatus KN_DoIOOperation(AudioServerPlugInDriverRef, AudioObjectID, AudioObjectID, UInt32, UInt32, UInt32, const AudioServerPlugInIOCycleInfo *, void *, void *);
static OSStatus KN_EndIOOperation(AudioServerPlugInDriverRef, AudioObjectID, UInt32, UInt32, UInt32, const AudioServerPlugInIOCycleInfo *);

static AudioServerPlugInDriverInterface gInterface = {
    NULL,
    KN_QueryInterface,
    KN_AddRef,
    KN_Release,
    KN_Initialize,
    KN_CreateDevice,
    KN_DestroyDevice,
    KN_AddDeviceClient,
    KN_RemoveDeviceClient,
    KN_PerformDeviceConfigurationChange,
    KN_AbortDeviceConfigurationChange,
    KN_HasProperty,
    KN_IsPropertySettable,
    KN_GetPropertyDataSize,
    KN_GetPropertyData,
    KN_SetPropertyData,
    KN_StartIO,
    KN_StopIO,
    KN_GetZeroTimeStamp,
    KN_WillDoIOOperation,
    KN_BeginIOOperation,
    KN_DoIOOperation,
    KN_EndIOOperation,
};
static AudioServerPlugInDriverInterface *gInterfacePtr = &gInterface;
static AudioServerPlugInDriverRef gDriverRef = &gInterfacePtr;

/// Point d'entrée nommé dans Info.plist (CFPlugInFactories).
void *KyberNowAudio_Create(CFAllocatorRef inAllocator, CFUUIDRef inRequestedTypeUUID) {
    (void)inAllocator;
    if (CFEqual(inRequestedTypeUUID, kAudioServerPlugInTypeUUID)) return gDriverRef;
    return NULL;
}

static HRESULT KN_QueryInterface(void *inDriver, REFIID inUUID, LPVOID *outInterface) {
    if (inDriver != gDriverRef || outInterface == NULL) return kAudioHardwareBadObjectError;
    CFUUIDRef requested = CFUUIDCreateFromUUIDBytes(NULL, inUUID);
    HRESULT result = E_NOINTERFACE;
    if (CFEqual(requested, IUnknownUUID) || CFEqual(requested, kAudioServerPlugInDriverInterfaceUUID)) {
        pthread_mutex_lock(&gStateMutex);
        gRefCount++;
        pthread_mutex_unlock(&gStateMutex);
        *outInterface = gDriverRef;
        result = S_OK;
    }
    CFRelease(requested);
    return result;
}

static ULONG KN_AddRef(void *inDriver) {
    if (inDriver != gDriverRef) return 0;
    pthread_mutex_lock(&gStateMutex);
    ULONG r = ++gRefCount;
    pthread_mutex_unlock(&gStateMutex);
    return r;
}

static ULONG KN_Release(void *inDriver) {
    if (inDriver != gDriverRef) return 0;
    pthread_mutex_lock(&gStateMutex);
    if (gRefCount > 0) gRefCount--;
    ULONG r = gRefCount;
    pthread_mutex_unlock(&gStateMutex);
    return r;
}

static OSStatus KN_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost) {
    if (inDriver != gDriverRef) return kAudioHardwareBadObjectError;
    gHost = inHost;
    UpdateTicksPerFrame();
    return 0;
}

static OSStatus KN_CreateDevice(AudioServerPlugInDriverRef d, CFDictionaryRef desc, const AudioServerPlugInClientInfo *ci, AudioObjectID *out) {
    (void)d; (void)desc; (void)ci; (void)out;
    return kAudioHardwareUnsupportedOperationError;
}

static OSStatus KN_DestroyDevice(AudioServerPlugInDriverRef d, AudioObjectID id) {
    (void)d; (void)id;
    return kAudioHardwareUnsupportedOperationError;
}

static OSStatus KN_AddDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID id, const AudioServerPlugInClientInfo *ci) {
    (void)ci;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    return 0;
}

static OSStatus KN_RemoveDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID id, const AudioServerPlugInClientInfo *ci) {
    (void)ci;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    return 0;
}

static OSStatus KN_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID id, UInt64 action, void *info) {
    (void)action; (void)info;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    pthread_mutex_lock(&gStateMutex);
    if (gPendingSampleRate > 0.0) gSampleRate = gPendingSampleRate;
    if (gPendingChannels > 0) gChannels = gPendingChannels;
    gPendingSampleRate = 0.0;
    gPendingChannels = 0;
    pthread_mutex_lock(&gIOMutex);
    UpdateTicksPerFrame();
    memset(gRing, 0, sizeof(gRing));
    pthread_mutex_unlock(&gIOMutex);
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus KN_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID id, UInt64 action, void *info) {
    (void)action; (void)info;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    pthread_mutex_lock(&gStateMutex);
    gPendingSampleRate = 0.0;
    gPendingChannels = 0;
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

// ----------------------------------------------------------------------------
// Propriétés

static bool IsStream(AudioObjectID id) { return id == kObjectStreamInput || id == kObjectStreamOutput; }

/// Disposition des voies pour le nombre de canaux (ordre WAV, celui de Kyber).
static AudioChannelLayoutTag LayoutTag(UInt32 channels) {
    switch (channels) {
    case 6: return kAudioChannelLayoutTag_WAVE_5_1_B;  // L R C LFE Rls Rrs (ordre WAV)
    case 8: return kAudioChannelLayoutTag_WAVE_7_1;    // L R C LFE Rls Rrs Ls Rs (ordre WAV)
    default: return kAudioChannelLayoutTag_Stereo;
    }
}

static Boolean KN_HasProperty(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid, const AudioObjectPropertyAddress *a) {
    (void)pid;
    if (d != gDriverRef || a == NULL) return false;
    UInt32 size = 0;
    return KN_GetPropertyDataSize(d, id, pid, a, 0, NULL, &size) == 0;
}

static OSStatus KN_IsPropertySettable(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid, const AudioObjectPropertyAddress *a, Boolean *out) {
    if (d != gDriverRef || a == NULL || out == NULL) return kAudioHardwareIllegalOperationError;
    if (!KN_HasProperty(d, id, pid, a)) return kAudioHardwareUnknownPropertyError;
    *out = false;
    if (id == kObjectDevice && a->mSelector == kAudioDevicePropertyNominalSampleRate) *out = true;
    if (IsStream(id) && (a->mSelector == kAudioStreamPropertyVirtualFormat || a->mSelector == kAudioStreamPropertyPhysicalFormat ||
                         a->mSelector == kAudioStreamPropertyIsActive))
        *out = true;
    return 0;
}

static OSStatus KN_GetPropertyDataSize(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid, const AudioObjectPropertyAddress *a,
                                       UInt32 qs, const void *q, UInt32 *out) {
    (void)pid; (void)qs; (void)q;
    if (d != gDriverRef || a == NULL || out == NULL) return kAudioHardwareIllegalOperationError;
    switch (id) {
    case kObjectPlugIn:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass:
        case kAudioObjectPropertyClass: *out = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: *out = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyManufacturer: *out = sizeof(CFStringRef); return 0;
        case kAudioObjectPropertyOwnedObjects:
        case kAudioPlugInPropertyDeviceList: *out = sizeof(AudioObjectID); return 0;
        case kAudioPlugInPropertyTranslateUIDToDevice: *out = sizeof(AudioObjectID); return 0;
        case kAudioPlugInPropertyResourceBundle: *out = sizeof(CFStringRef); return 0;
        }
        return kAudioHardwareUnknownPropertyError;
    case kObjectDevice:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass:
        case kAudioObjectPropertyClass: *out = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: *out = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyName:
        case kAudioObjectPropertyManufacturer:
        case kAudioDevicePropertyDeviceUID:
        case kAudioDevicePropertyModelUID: *out = sizeof(CFStringRef); return 0;
        case kAudioDevicePropertyTransportType:
        case kAudioDevicePropertyClockDomain:
        case kAudioDevicePropertyDeviceIsAlive:
        case kAudioDevicePropertyDeviceIsRunning:
        case kAudioDevicePropertyDeviceCanBeDefaultDevice:
        case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
        case kAudioDevicePropertyLatency:
        case kAudioDevicePropertySafetyOffset:
        case kAudioDevicePropertyIsHidden:
        case kAudioDevicePropertyZeroTimeStampPeriod:
        case kAudioDevicePropertyClockIsStable: *out = sizeof(UInt32); return 0;
        case kAudioDevicePropertyRelatedDevices: *out = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyOwnedObjects:
        case kAudioDevicePropertyStreams:
            if (a->mScope == kAudioObjectPropertyScopeGlobal) *out = 2 * sizeof(AudioObjectID);
            else *out = sizeof(AudioObjectID);
            return 0;
        case kAudioObjectPropertyControlList: *out = 0; return 0;
        case kAudioDevicePropertyNominalSampleRate: *out = sizeof(Float64); return 0;
        case kAudioDevicePropertyAvailableNominalSampleRates: *out = kSampleRateCount * sizeof(AudioValueRange); return 0;
        case kAudioDevicePropertyPreferredChannelsForStereo: *out = 2 * sizeof(UInt32); return 0;
        case kAudioDevicePropertyPreferredChannelLayout: *out = offsetof(AudioChannelLayout, mChannelDescriptions); return 0;
        }
        return kAudioHardwareUnknownPropertyError;
    case kObjectStreamInput:
    case kObjectStreamOutput:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass:
        case kAudioObjectPropertyClass: *out = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: *out = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyOwnedObjects: *out = 0; return 0;
        case kAudioObjectPropertyName: *out = sizeof(CFStringRef); return 0;
        case kAudioStreamPropertyIsActive:
        case kAudioStreamPropertyDirection:
        case kAudioStreamPropertyTerminalType:
        case kAudioStreamPropertyStartingChannel:
        case kAudioStreamPropertyLatency: *out = sizeof(UInt32); return 0;
        case kAudioStreamPropertyVirtualFormat:
        case kAudioStreamPropertyPhysicalFormat: *out = sizeof(AudioStreamBasicDescription); return 0;
        case kAudioStreamPropertyAvailableVirtualFormats:
        case kAudioStreamPropertyAvailablePhysicalFormats:
            *out = kSampleRateCount * kChannelCountCount * sizeof(AudioStreamRangedDescription);
            return 0;
        }
        return kAudioHardwareUnknownPropertyError;
    }
    return kAudioHardwareBadObjectError;
}

#define NEED(sz) do { if (inDataSize < (sz)) return kAudioHardwareBadPropertySizeError; } while (0)

static OSStatus KN_GetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid, const AudioObjectPropertyAddress *a,
                                   UInt32 qs, const void *q, UInt32 inDataSize, UInt32 *outSize, void *outData) {
    (void)pid;
    if (d != gDriverRef || a == NULL || outSize == NULL || outData == NULL) return kAudioHardwareIllegalOperationError;
    pthread_mutex_lock(&gStateMutex);
    Float64 rate = gSampleRate;
    UInt32 channels = gChannels;
    UInt32 running = gIORunning;
    bool inputActive = gInputActive, outputActive = gOutputActive;
    pthread_mutex_unlock(&gStateMutex);

    switch (id) {
    case kObjectPlugIn:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioObjectClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioPlugInClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: NEED(sizeof(AudioObjectID)); *(AudioObjectID *)outData = kAudioObjectUnknown; *outSize = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyManufacturer: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(kManufacturer); *outSize = sizeof(CFStringRef); return 0;
        case kAudioObjectPropertyOwnedObjects:
        case kAudioPlugInPropertyDeviceList:
            *outSize = 0;
            if (inDataSize >= sizeof(AudioObjectID)) { *(AudioObjectID *)outData = kObjectDevice; *outSize = sizeof(AudioObjectID); }
            return 0;
        case kAudioPlugInPropertyTranslateUIDToDevice: {
            NEED(sizeof(AudioObjectID));
            AudioObjectID result = kAudioObjectUnknown;
            if (qs == sizeof(CFStringRef) && q != NULL && CFStringCompare(*(CFStringRef *)q, CFSTR(kDeviceUID), 0) == kCFCompareEqualTo)
                result = kObjectDevice;
            *(AudioObjectID *)outData = result;
            *outSize = sizeof(AudioObjectID);
            return 0;
        }
        case kAudioPlugInPropertyResourceBundle: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(""); *outSize = sizeof(CFStringRef); return 0;
        }
        return kAudioHardwareUnknownPropertyError;

    case kObjectDevice:
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioObjectClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioDeviceClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: NEED(sizeof(AudioObjectID)); *(AudioObjectID *)outData = kObjectPlugIn; *outSize = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyName: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(kDeviceName); *outSize = sizeof(CFStringRef); return 0;
        case kAudioObjectPropertyManufacturer: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(kManufacturer); *outSize = sizeof(CFStringRef); return 0;
        case kAudioDevicePropertyDeviceUID: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(kDeviceUID); *outSize = sizeof(CFStringRef); return 0;
        case kAudioDevicePropertyModelUID: NEED(sizeof(CFStringRef)); *(CFStringRef *)outData = CFSTR(kModelUID); *outSize = sizeof(CFStringRef); return 0;
        case kAudioDevicePropertyTransportType: NEED(sizeof(UInt32)); *(UInt32 *)outData = kAudioDeviceTransportTypeVirtual; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyRelatedDevices:
            *outSize = 0;
            if (inDataSize >= sizeof(AudioObjectID)) { *(AudioObjectID *)outData = kObjectDevice; *outSize = sizeof(AudioObjectID); }
            return 0;
        case kAudioDevicePropertyClockDomain: NEED(sizeof(UInt32)); *(UInt32 *)outData = 0; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyDeviceIsAlive: NEED(sizeof(UInt32)); *(UInt32 *)outData = 1; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyDeviceIsRunning: NEED(sizeof(UInt32)); *(UInt32 *)outData = running > 0; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyDeviceCanBeDefaultDevice:
            // Jamais le micro par défaut : seulement une sortie.
            NEED(sizeof(UInt32)); *(UInt32 *)outData = a->mScope != kAudioObjectPropertyScopeInput; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
            NEED(sizeof(UInt32)); *(UInt32 *)outData = a->mScope != kAudioObjectPropertyScopeInput; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyLatency: NEED(sizeof(UInt32)); *(UInt32 *)outData = kLatency; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertySafetyOffset: NEED(sizeof(UInt32)); *(UInt32 *)outData = kSafetyOffset; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyIsHidden: NEED(sizeof(UInt32)); *(UInt32 *)outData = 0; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyZeroTimeStampPeriod: NEED(sizeof(UInt32)); *(UInt32 *)outData = kRingFrames; *outSize = sizeof(UInt32); return 0;
        case kAudioDevicePropertyClockIsStable: NEED(sizeof(UInt32)); *(UInt32 *)outData = 1; *outSize = sizeof(UInt32); return 0;
        case kAudioObjectPropertyOwnedObjects:
        case kAudioDevicePropertyStreams: {
            AudioObjectID ids[2];
            UInt32 n = 0;
            if (a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeInput) ids[n++] = kObjectStreamInput;
            if (a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeOutput) ids[n++] = kObjectStreamOutput;
            UInt32 fit = inDataSize / sizeof(AudioObjectID);
            if (n > fit) n = fit;
            memcpy(outData, ids, n * sizeof(AudioObjectID));
            *outSize = n * sizeof(AudioObjectID);
            return 0;
        }
        case kAudioObjectPropertyControlList: *outSize = 0; return 0;
        case kAudioDevicePropertyNominalSampleRate: NEED(sizeof(Float64)); *(Float64 *)outData = rate; *outSize = sizeof(Float64); return 0;
        case kAudioDevicePropertyAvailableNominalSampleRates: {
            UInt32 n = inDataSize / sizeof(AudioValueRange);
            if (n > kSampleRateCount) n = kSampleRateCount;
            AudioValueRange *r = outData;
            for (UInt32 i = 0; i < n; i++) { r[i].mMinimum = kSampleRates[i]; r[i].mMaximum = kSampleRates[i]; }
            *outSize = n * sizeof(AudioValueRange);
            return 0;
        }
        case kAudioDevicePropertyPreferredChannelsForStereo:
            NEED(2 * sizeof(UInt32));
            ((UInt32 *)outData)[0] = 1;
            ((UInt32 *)outData)[1] = 2;
            *outSize = 2 * sizeof(UInt32);
            return 0;
        case kAudioDevicePropertyPreferredChannelLayout: {
            UInt32 sz = offsetof(AudioChannelLayout, mChannelDescriptions);
            NEED(sz);
            AudioChannelLayout *l = outData;
            memset(l, 0, sz);
            l->mChannelLayoutTag = LayoutTag(channels);
            *outSize = sz;
            return 0;
        }
        }
        return kAudioHardwareUnknownPropertyError;

    case kObjectStreamInput:
    case kObjectStreamOutput: {
        bool input = id == kObjectStreamInput;
        switch (a->mSelector) {
        case kAudioObjectPropertyBaseClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioObjectClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyClass: NEED(sizeof(AudioClassID)); *(AudioClassID *)outData = kAudioStreamClassID; *outSize = sizeof(AudioClassID); return 0;
        case kAudioObjectPropertyOwner: NEED(sizeof(AudioObjectID)); *(AudioObjectID *)outData = kObjectDevice; *outSize = sizeof(AudioObjectID); return 0;
        case kAudioObjectPropertyOwnedObjects: *outSize = 0; return 0;
        case kAudioObjectPropertyName:
            NEED(sizeof(CFStringRef));
            *(CFStringRef *)outData = input ? CFSTR("KyberNow (retour)") : CFSTR("KyberNow");
            *outSize = sizeof(CFStringRef);
            return 0;
        case kAudioStreamPropertyIsActive: NEED(sizeof(UInt32)); *(UInt32 *)outData = input ? inputActive : outputActive; *outSize = sizeof(UInt32); return 0;
        case kAudioStreamPropertyDirection: NEED(sizeof(UInt32)); *(UInt32 *)outData = input ? 1 : 0; *outSize = sizeof(UInt32); return 0;
        case kAudioStreamPropertyTerminalType:
            NEED(sizeof(UInt32));
            *(UInt32 *)outData = input ? kAudioStreamTerminalTypeLine : kAudioStreamTerminalTypeSpeaker;
            *outSize = sizeof(UInt32);
            return 0;
        case kAudioStreamPropertyStartingChannel: NEED(sizeof(UInt32)); *(UInt32 *)outData = 1; *outSize = sizeof(UInt32); return 0;
        case kAudioStreamPropertyLatency: NEED(sizeof(UInt32)); *(UInt32 *)outData = kLatency; *outSize = sizeof(UInt32); return 0;
        case kAudioStreamPropertyVirtualFormat:
        case kAudioStreamPropertyPhysicalFormat:
            NEED(sizeof(AudioStreamBasicDescription));
            *(AudioStreamBasicDescription *)outData = FormatFor(rate, channels);
            *outSize = sizeof(AudioStreamBasicDescription);
            return 0;
        case kAudioStreamPropertyAvailableVirtualFormats:
        case kAudioStreamPropertyAvailablePhysicalFormats: {
            UInt32 total = kSampleRateCount * kChannelCountCount;
            UInt32 n = inDataSize / sizeof(AudioStreamRangedDescription);
            if (n > total) n = total;
            AudioStreamRangedDescription *r = outData;
            for (UInt32 i = 0; i < n; i++) {
                Float64 sr = kSampleRates[i / kChannelCountCount];
                r[i].mFormat = FormatFor(sr, kChannelCounts[i % kChannelCountCount]);
                r[i].mSampleRateRange.mMinimum = sr;
                r[i].mSampleRateRange.mMaximum = sr;
            }
            *outSize = n * sizeof(AudioStreamRangedDescription);
            return 0;
        }
        }
        return kAudioHardwareUnknownPropertyError;
    }
    }
    return kAudioHardwareBadObjectError;
}

/// Demande un changement de format (nombre de canaux, fréquence) : l'hôte
/// arrête les entrées-sorties et appelle PerformDeviceConfigurationChange.
static OSStatus RequestChange(Float64 rate, UInt32 channels) {
    pthread_mutex_lock(&gStateMutex);
    bool same = rate == gSampleRate && channels == gChannels;
    if (!same) {
        gPendingSampleRate = rate;
        gPendingChannels = channels;
    }
    pthread_mutex_unlock(&gStateMutex);
    if (same || gHost == NULL) return 0;
    dispatch_async(dispatch_get_global_queue(DISPATCH_QUEUE_PRIORITY_DEFAULT, 0), ^{
        gHost->RequestDeviceConfigurationChange(gHost, kObjectDevice, 0, NULL);
    });
    return 0;
}

static OSStatus KN_SetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid, const AudioObjectPropertyAddress *a,
                                   UInt32 qs, const void *q, UInt32 inDataSize, const void *inData) {
    (void)pid; (void)qs; (void)q;
    if (d != gDriverRef || a == NULL || inData == NULL) return kAudioHardwareIllegalOperationError;
    if (id == kObjectDevice && a->mSelector == kAudioDevicePropertyNominalSampleRate) {
        if (inDataSize != sizeof(Float64)) return kAudioHardwareBadPropertySizeError;
        Float64 rate = *(const Float64 *)inData;
        if (!SupportedRate(rate)) return kAudioHardwareIllegalOperationError;
        pthread_mutex_lock(&gStateMutex);
        UInt32 channels = gChannels;
        pthread_mutex_unlock(&gStateMutex);
        return RequestChange(rate, channels);
    }
    if (IsStream(id) && (a->mSelector == kAudioStreamPropertyVirtualFormat || a->mSelector == kAudioStreamPropertyPhysicalFormat)) {
        if (inDataSize != sizeof(AudioStreamBasicDescription)) return kAudioHardwareBadPropertySizeError;
        const AudioStreamBasicDescription *f = inData;
        if (f->mFormatID != kAudioFormatLinearPCM || !SupportedChannels(f->mChannelsPerFrame) || !SupportedRate(f->mSampleRate))
            return kAudioDeviceUnsupportedFormatError;
        return RequestChange(f->mSampleRate, f->mChannelsPerFrame);
    }
    if (IsStream(id) && a->mSelector == kAudioStreamPropertyIsActive) {
        if (inDataSize != sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
        pthread_mutex_lock(&gStateMutex);
        if (id == kObjectStreamInput) gInputActive = *(const UInt32 *)inData != 0;
        else gOutputActive = *(const UInt32 *)inData != 0;
        pthread_mutex_unlock(&gStateMutex);
        AudioObjectPropertyAddress changed = {kAudioStreamPropertyIsActive, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
        if (gHost) gHost->PropertiesChanged(gHost, id, 1, &changed);
        return 0;
    }
    return kAudioHardwareUnknownPropertyError;
}

// ----------------------------------------------------------------------------
// Entrées-sorties

static OSStatus KN_StartIO(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client) {
    (void)client;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    pthread_mutex_lock(&gStateMutex);
    if (gIORunning == 0) {
        pthread_mutex_lock(&gIOMutex);
        gNumberTimeStamps = 0;
        gAnchorHostTime = mach_absolute_time();
        memset(gRing, 0, sizeof(gRing));
        pthread_mutex_unlock(&gIOMutex);
    }
    gIORunning++;
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus KN_StopIO(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client) {
    (void)client;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    pthread_mutex_lock(&gStateMutex);
    if (gIORunning > 0) gIORunning--;
    pthread_mutex_unlock(&gStateMutex);
    return 0;
}

static OSStatus KN_GetZeroTimeStamp(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, Float64 *outSampleTime, UInt64 *outHostTime,
                                    UInt64 *outSeed) {
    (void)client;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    pthread_mutex_lock(&gIOMutex);
    UInt64 now = mach_absolute_time();
    Float64 ticksPerRing = gHostTicksPerFrame * (Float64)kRingFrames;
    UInt64 next = gAnchorHostTime + (UInt64)((Float64)(gNumberTimeStamps + 1) * ticksPerRing);
    if (next <= now) gNumberTimeStamps++;
    *outSampleTime = (Float64)(gNumberTimeStamps * kRingFrames);
    *outHostTime = gAnchorHostTime + (UInt64)((Float64)gNumberTimeStamps * ticksPerRing);
    *outSeed = 1;
    pthread_mutex_unlock(&gIOMutex);
    return 0;
}

static OSStatus KN_WillDoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op, Boolean *outWill,
                                     Boolean *outInPlace) {
    (void)client;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    bool will = op == kAudioServerPlugInIOOperationReadInput || op == kAudioServerPlugInIOOperationWriteMix;
    if (outWill) *outWill = will;
    if (outInPlace) *outInPlace = true;
    return 0;
}

static OSStatus KN_BeginIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op, UInt32 frames,
                                    const AudioServerPlugInIOCycleInfo *info) {
    (void)client; (void)op; (void)frames; (void)info;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    return 0;
}

static OSStatus KN_DoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, AudioObjectID stream, UInt32 client, UInt32 op, UInt32 frames,
                                 const AudioServerPlugInIOCycleInfo *info, void *main, void *secondary) {
    (void)client; (void)secondary;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    if (main == NULL || frames == 0 || frames > kRingFrames) return 0;
    UInt32 channels = gChannels;
    Float32 *buf = main;
    if (op == kAudioServerPlugInIOOperationWriteMix && stream == kObjectStreamOutput) {
        UInt64 start = (UInt64)info->mOutputTime.mSampleTime % kRingFrames;
        UInt32 first = (UInt32)((start + frames <= kRingFrames) ? frames : kRingFrames - start);
        memcpy(&gRing[start * channels], buf, (size_t)first * channels * sizeof(Float32));
        if (first < frames) memcpy(gRing, buf + (size_t)first * channels, (size_t)(frames - first) * channels * sizeof(Float32));
    } else if (op == kAudioServerPlugInIOOperationReadInput && stream == kObjectStreamInput) {
        // Lu puis vidé : rien de joué, rien de rendu.
        UInt64 start = (UInt64)info->mInputTime.mSampleTime % kRingFrames;
        UInt32 first = (UInt32)((start + frames <= kRingFrames) ? frames : kRingFrames - start);
        memcpy(buf, &gRing[start * channels], (size_t)first * channels * sizeof(Float32));
        memset(&gRing[start * channels], 0, (size_t)first * channels * sizeof(Float32));
        if (first < frames) {
            memcpy(buf + (size_t)first * channels, gRing, (size_t)(frames - first) * channels * sizeof(Float32));
            memset(gRing, 0, (size_t)(frames - first) * channels * sizeof(Float32));
        }
    }
    return 0;
}

static OSStatus KN_EndIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op, UInt32 frames,
                                  const AudioServerPlugInIOCycleInfo *info) {
    (void)client; (void)op; (void)frames; (void)info;
    if (d != gDriverRef || id != kObjectDevice) return kAudioHardwareBadObjectError;
    return 0;
}
