print("capacités :", DecoderCapabilities.current.summary, "hevc444:", DecoderCapabilities.current.hevc444)
run("yuv420p.hevc", codec: 0x68323635)
run("yuv444p.hevc", codec: 0x68323635)
run("h264.h264", codec: 0x68323634)
