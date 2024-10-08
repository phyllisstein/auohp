(ns whisper-clj.core
  (:import [io.github.ggerganov.whispercpp WhisperCpp]
           [javax.sound.sampled AudioSystem]))


(defn -main
  "Main entry point for the application"
  [& args]
  (println "Initializing WhisperCpp")
  (let [model-path (.toPath (clojure.java.io/file "resources/ggml-tiny.en.bin"))
        dylib-path (.toPath (clojure.java.io/file "resources/libwhisper.dylib"))
        load-options (WhisperJNI$LoadOptions.)]
    (set! (. load-options -whisperLib) dylib-path)
    (WhisperJNI/loadLibrary load-options)
    (println "Whisper initialized")
    (let [whisper (WhisperJNI.)
          whisper-ctx (.init whisper model-path)
          whisper-params (WhisperFullParams.)]
      (println "Whisper initialized")
      ;; (set! (. whisper-params -detectLanguage) false)
      ;; (set! (. whisper-params -speedUp) true)
      ;; (set! (. whisper-params -suppressBlank) true)
      ;; (set! (. whisper-params -noContext) false)
      (let [audio-input-stream (AudioSystem/getAudioInputStream (clojure.java.io/file "resources/004.wav"))]
        (println "Audio input stream loaded")
        (let [stream-size (.available audio-input-stream)
              audio-data (byte-array stream-size)
              audio (float-array audio-data)]
          (println "Audio data loaded")
          (println "Stream size: " stream-size)
          (println "Byte size: " (count audio-data))
          (println "Float size: " (count audio))
          (.read audio-input-stream audio-data 0 stream-size)
          (let [result (.full whisper whisper-ctx whisper-params audio stream-size)
                result-count (.fullNSegments whisper whisper-ctx)]
            (println "Whisper processed this many statements: " result-count)
            (println "0 said: " (.fullGetSegmentText whisper whisper-ctx (- result-count 0)))
            (println "13 said: " (.fullGetSegmentText whisper whisper-ctx (- result-count 13)))
            (println "69 said: " (.fullGetSegmentText whisper whisper-ctx (- result-count 69)))
            (println "Full text: ")))))))
