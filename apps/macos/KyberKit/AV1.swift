// AV1 pour VideoToolbox : Kyber envoie le flux de l'encodeur tel quel (OBU
// « low overhead », l'en-tête de séquence en paquet de configuration et
// répété à chaque image clé). VideoToolbox veut, lui, une description
// `av1C` (AV1-ISOBMFF §2.3) tirée de l'en-tête de séquence, et des
// échantillons sans délimiteur temporel, chaque OBU avec sa taille.

import CoreMedia
import Foundation

enum AV1 {
    static let obuSequenceHeader: UInt8 = 1
    static let obuTemporalDelimiter: UInt8 = 2
    static let obuPadding: UInt8 = 15

    struct OBU {
        let type: UInt8
        /// L'OBU entier (en-tête compris), avec son champ de taille.
        let bytes: Data
        /// La charge utile seule.
        let payload: Data
    }

    /// Découpe un flux d'OBU. Un OBU sans champ de taille occupe le reste
    /// du paquet ; on le réécrit avec sa taille (exigée par VideoToolbox).
    static func obus(_ data: UnsafeBufferPointer<UInt8>) -> [OBU] {
        var out: [OBU] = []
        var i = 0
        let n = data.count
        while i < n {
            let header = data[i]
            let type = (header >> 3) & 0x0F
            let hasExtension = header & 0x04 != 0
            let hasSize = header & 0x02 != 0
            var p = i + 1 + (hasExtension ? 1 : 0)
            guard p <= n else { break }
            var size = n - p
            if hasSize {
                guard let (value, used) = leb128(data, at: p) else { break }
                size = Int(value)
                p += used
            }
            guard p + size <= n else { break }
            let headerBytes = Data(UnsafeBufferPointer(rebasing: data[i..<(i + 1 + (hasExtension ? 1 : 0))]))
            let payload = Data(UnsafeBufferPointer(rebasing: data[p..<(p + size)]))
            var bytes = Data()
            bytes.append(headerBytes[0] | 0x02)
            if hasExtension { bytes.append(headerBytes[1]) }
            bytes.append(encodeLeb128(UInt64(size)))
            bytes.append(payload)
            out.append(OBU(type: type, bytes: bytes, payload: payload))
            i = p + size
        }
        return out
    }

    static func leb128(_ data: UnsafeBufferPointer<UInt8>, at start: Int) -> (UInt64, Int)? {
        var value: UInt64 = 0
        for k in 0..<8 {
            guard start + k < data.count else { return nil }
            let byte = data[start + k]
            value |= UInt64(byte & 0x7F) << (7 * UInt64(k))
            if byte & 0x80 == 0 { return (value, k + 1) }
        }
        return nil
    }

    static func encodeLeb128(_ value: UInt64) -> Data {
        var v = value
        var out = Data()
        repeat {
            var byte = UInt8(v & 0x7F)
            v >>= 7
            if v != 0 { byte |= 0x80 }
            out.append(byte)
        } while v != 0
        return out
    }

    /// Ce que l'en-tête de séquence apprend (AV1 §5.5).
    struct SequenceHeader {
        var profile: UInt8 = 0
        var level: UInt8 = 0
        var tier: UInt8 = 0
        var highBitdepth = false
        var twelveBit = false
        var monochrome = false
        var subsamplingX = true
        var subsamplingY = true
        var chromaSamplePosition: UInt8 = 0
        var width = 0
        var height = 0
        var fullRange = false
        var matrix: UInt8 = 2 // non précisé
    }

    static func parseSequenceHeader(_ payload: Data) -> SequenceHeader? {
        var r = BitReader(payload)
        var h = SequenceHeader()
        h.profile = UInt8(r.bits(3))
        _ = r.bit() // still_picture
        let reduced = r.bit()
        var decoderModelInfo = false
        var bufferDelayLength = 0
        if reduced {
            h.level = UInt8(r.bits(5))
        } else {
            if r.bit() { // timing_info_present_flag
                _ = r.bits(32); _ = r.bits(32)
                if r.bit() { _ = r.uvlc() } // equal_picture_interval
                decoderModelInfo = r.bit()
                if decoderModelInfo {
                    bufferDelayLength = Int(r.bits(5)) + 1
                    _ = r.bits(32); _ = r.bits(5); _ = r.bits(5)
                }
            }
            let initialDisplayDelay = r.bit()
            let count = Int(r.bits(5)) + 1
            for i in 0..<count {
                _ = r.bits(12) // operating_point_idc
                let level = UInt8(r.bits(5))
                let tier: UInt8 = level > 7 ? (r.bit() ? 1 : 0) : 0
                if i == 0 { h.level = level; h.tier = tier }
                if decoderModelInfo, r.bit() {
                    _ = r.bits(bufferDelayLength); _ = r.bits(bufferDelayLength); _ = r.bit()
                }
                if initialDisplayDelay, r.bit() { _ = r.bits(4) }
            }
        }
        let widthBits = Int(r.bits(4)) + 1
        let heightBits = Int(r.bits(4)) + 1
        h.width = Int(r.bits(widthBits)) + 1
        h.height = Int(r.bits(heightBits)) + 1
        if !reduced, r.bit() { _ = r.bits(4); _ = r.bits(3) } // frame_id_numbers_present_flag
        _ = r.bit(); _ = r.bit(); _ = r.bit() // 128x128, filter_intra, intra_edge
        if !reduced {
            _ = r.bit(); _ = r.bit(); _ = r.bit(); _ = r.bit() // interintra, masked, warped, dual_filter
            let orderHint = r.bit()
            if orderHint { _ = r.bit(); _ = r.bit() } // jnt_comp, ref_frame_mvs
            let chooseScreenContent = r.bit()
            let forceScreenContent: UInt32 = chooseScreenContent ? 2 : (r.bit() ? 1 : 0)
            if forceScreenContent > 0, !r.bit() { _ = r.bit() } // seq_choose_integer_mv / force
            if orderHint { _ = r.bits(3) }
        }
        _ = r.bit(); _ = r.bit(); _ = r.bit() // superres, cdef, restoration
        // color_config
        h.highBitdepth = r.bit()
        if h.profile == 2, h.highBitdepth { h.twelveBit = r.bit() }
        h.monochrome = h.profile == 1 ? false : r.bit()
        var primaries: UInt32 = 2, transfer: UInt32 = 2, matrix: UInt32 = 2
        if r.bit() { primaries = r.bits(8); transfer = r.bits(8); matrix = r.bits(8) }
        h.matrix = UInt8(matrix)
        if h.monochrome {
            h.fullRange = r.bit()
            h.subsamplingX = true; h.subsamplingY = true
        } else if primaries == 1, transfer == 13, matrix == 0 {
            h.fullRange = true
            h.subsamplingX = false; h.subsamplingY = false
        } else {
            h.fullRange = r.bit()
            switch h.profile {
            case 0: h.subsamplingX = true; h.subsamplingY = true
            case 1: h.subsamplingX = false; h.subsamplingY = false
            default:
                if h.twelveBit {
                    h.subsamplingX = r.bit()
                    h.subsamplingY = h.subsamplingX ? r.bit() : false
                } else {
                    h.subsamplingX = true; h.subsamplingY = false
                }
            }
            if h.subsamplingX, h.subsamplingY { h.chromaSamplePosition = UInt8(r.bits(2)) }
        }
        return r.overrun ? nil : h
    }

    /// Description `av1C` : 4 octets de résumé, puis l'OBU d'en-tête de séquence.
    static func av1C(_ h: SequenceHeader, sequenceHeaderOBU: Data) -> Data {
        var d = Data([0x81]) // marker + version 1
        d.append((h.profile << 5) | (h.level & 0x1F))
        var b: UInt8 = h.tier << 7
        if h.highBitdepth { b |= 0x40 }
        if h.twelveBit { b |= 0x20 }
        if h.monochrome { b |= 0x10 }
        if h.subsamplingX { b |= 0x08 }
        if h.subsamplingY { b |= 0x04 }
        b |= h.chromaSamplePosition & 0x03
        d.append(b)
        d.append(0) // initial_presentation_delay absent
        d.append(sequenceHeaderOBU)
        return d
    }

    static func format(_ h: SequenceHeader, sequenceHeaderOBU: Data) -> CMVideoFormatDescription? {
        let atoms: [CFString: Any] = ["av1C" as CFString: av1C(h, sequenceHeaderOBU: sequenceHeaderOBU)]
        var extensions: [CFString: Any] = [kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms: atoms]
        if h.fullRange { extensions[kCMFormatDescriptionExtension_FullRangeVideo] = true }
        var format: CMVideoFormatDescription?
        let status = CMVideoFormatDescriptionCreate(
            allocator: kCFAllocatorDefault, codecType: kCMVideoCodecType_AV1,
            width: Int32(h.width), height: Int32(h.height),
            extensions: extensions as CFDictionary, formatDescriptionOut: &format
        )
        return status == noErr ? format : nil
    }
}

/// Lecture bit à bit, poids fort d'abord.
struct BitReader {
    private let data: Data
    private var position = 0
    private(set) var overrun = false

    init(_ data: Data) { self.data = data }

    mutating func bit() -> Bool { bits(1) == 1 }

    mutating func bits(_ count: Int) -> UInt32 {
        var value: UInt32 = 0
        for _ in 0..<count {
            let byte = position / 8
            guard byte < data.count else { overrun = true; return value }
            let bit = (data[data.startIndex + byte] >> (7 - UInt8(position % 8))) & 1
            value = (value << 1) | UInt32(bit)
            position += 1
        }
        return value
    }

    mutating func uvlc() -> UInt32 {
        var zeros = 0
        while !bit() {
            zeros += 1
            if zeros >= 32 || overrun { return 0 }
        }
        return zeros == 0 ? 0 : bits(zeros) + (1 << zeros) - 1
    }
}
