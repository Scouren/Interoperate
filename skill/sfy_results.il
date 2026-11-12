;; sfy_results.il: a maestro history's results as a results dump (interop spec 4.2 history
;; and job records, 9.10). Prefix form only (spec 5.1).
;;
;; What only Virtuoso can do happens here: the point directories (axlGetPointPsfDir: its
;; directory number is not the design point) and the waveforms, which maestro writes as
;; psfxl that the psf utility cannot read, printed through OCEAN's drGetElem at %.17g
;; (bit-exact against Spectre's nutbin, G35). Values, verdicts and var values stay in the
;; history's .rdb (SQLite, full doubles), copied beside the dump for the Rust side to read:
;; maeGetOutputValue rounds to 7 digits.

;; Writes <dir>/<history>.sfyx (and its .log): sfyx, ("history" lib cell view history rdb
;; zip), one ("job" test corner point psfdir waves) per simulated point (Nominal is corner
;; "", G38), end; beside it <history>.rdb, <history>.zip (the history's own setup) and
;; <history>.<n>.txt, the waveforms of job n. dir is absolute. t on PASS.
(procedure (SfyAdeResults dir lib cell history @key (view "maestro"))
  (let ((path (strcat dir "/" history ".sfyx")) (sfyLogPort nil)
        (sfyCountTab (makeTable "sfyCounts" nil)) (sfyCountKeys nil) (sfyFails 0)
        (sess nil) (sdb nil))
    (setq sfyLogPort (SfyLogOpen path))
    (SfyLog (sprintf nil "SfyAdeResults %s %s/%s/%s %s" dir lib cell view history))
    (SfyPreflight)
    (setq sess (car (errset (maeOpenSetup lib cell view ?mode "r"))))
    (when sess
      (setq sdb (car (errset (axlGetMainSetupDB sess))))
      ;; A read-only session answers setup database 0 (sfy_ade.il): open it in "a" mode,
      ;; save nothing, force-close.
      (when (or (null sdb) (equal sdb 0))
        (SfyAdeClose sess)
        (setq sess (car (errset (maeOpenSetup lib cell view ?mode "a"))))))
    (if (null sess)
      (SfyRefuse 0 (sprintf nil "cannot open %s/%s/%s" lib cell view))
      (unwindProtect
        (unless (errset (SfyResDump dir path lib cell view history sess) t)
          (SfyRefuse 0 (sprintf nil "SKILL error %s" (SfyErrMsg))))
        (SfyAdeClose sess)))
    (SfyLogClose)))

;; The dump of history in session sess.
(procedure (SfyResDump dir path lib cell view history sess)
  (let ((sdb (axlGetMainSetupDB sess)) (h nil) (rdb nil) (tail nil) (zip "") (jobs nil)
        (psf nil) (dp 0) (n 0) (wf nil) (port nil) (corners nil))
    (setq h (car (errset (axlGetHistoryEntry sdb history))))
    (setq rdb (and h (car (errset (axlGetHistoryResults h)))))
    (cond
      ((not (stringp rdb))
        (SfyRefuse 0 (sprintf nil "%s/%s/%s: no history %s" lib cell view history)))
      ((SfyResQuoted (list dir rdb))
        (SfyRefuse 0 (sprintf nil "a path holds a quote: %s %s" dir rdb)))
      (t
        (SfyResCopy rdb (strcat dir "/" history ".rdb"))
        ;; The setup the history ran: <view>/history/<history>.zip, the .rdb being
        ;; <view>/results/maestro/<history>.rdb (assembler, Directory Structure of a
        ;; Maestro Cellview).
        (setq tail (strcat "/results/maestro/" history ".rdb"))
        (when (and (greaterp (strlen rdb) (strlen tail))
                   (equal (substring rdb (add1 (difference (strlen rdb) (strlen tail)))) tail))
          (setq zip (strcat (substring rdb 1 (difference (strlen rdb) (strlen tail)))
                      "/history/" history ".zip"))
          (if (isFile zip)
            (progn
              (SfyResCopy zip (strcat dir "/" history ".zip"))
              (setq zip (strcat history ".zip")))
            (setq zip "")))
        ;; The corners a point can have: Nominal ("") and every corner of the setup
        ;; (ADE lists Nominal as "Nominal", which the point functions refuse, G38).
        (setq corners (cons "" (sort (setof c (copy (car (errset (maeGetSetup ?typeName "corners"
                                                                   ?session sess))))
                                       (nequal c "Nominal"))
                                 nil)))
        (foreach test (sort (copy (car (errset (maeGetSetup ?session sess)))) nil)
          (foreach c corners
            ;; Design points 1, 2, ... until one has no directory (at most 2^16).
            (setq dp 1)
            (setq psf (car (errset (axlGetPointPsfDir h test ?cornerName c ?designPointId dp))))
            (while (and (stringp psf) (isDir psf) (leqp dp 65536))
              (setq n (add1 n))
              (setq wf (sprintf nil "%s.%d.txt" history n))
              (unless (SfyResWaves psf (strcat dir "/" wf))
                (SfyLog (sprintf nil "%s %L point %d: no waveform in %s" test c dp psf))
                (setq wf ""))
              (setq jobs (cons (list test c dp psf wf) jobs))
              (SfyCount "job")
              (setq dp (add1 dp))
              (setq psf (car (errset (axlGetPointPsfDir h test ?cornerName c ?designPointId dp)))))))
        (setq port (outfile path "w"))
        (if (null port)
          (SfyRefuse 0 (sprintf nil "cannot write %s" path))
          (unwindProtect
            (progn
              (SfyPut port "sfyx" (list 1 (SfyProducer)))
              (SfyPut port "history" (list lib cell view history (strcat history ".rdb") zip))
              (foreach j (reverse jobs)
                (SfyPut port "job" j))
              (SfyPut port "end" nil))
            (close port)))))))

;; Whether a string of paths holds a single quote (the shell copy quotes with them).
(procedure (SfyResQuoted paths)
  (exists p paths (greaterp (length (parseString (strcat "x" p "x") "'")) 1)))

;; Copies file src to dst (paths without single quotes).
(procedure (SfyResCopy src dst)
  (SfyLog (sprintf nil "copy %s %s" src dst))
  (system (sprintf nil "cp '%s' '%s'" src dst)))

;; Writes every waveform of the point directory psf to file (the format of
;; virtuoso::results::wave): per OCEAN result "# analysis <name>", per waveform
;; "# <name> <n> points <V|I>" then n lines "x y" or "x re im". nil when nothing was
;; written (the directory does not open, or holds no waveform).
(procedure (SfyResWaves psf file)
  (let ((port nil) (any nil) (w nil) (kind nil))
    (when (car (errset (openResults psf)))
      (setq port (outfile file "w"))
      (when port
        (unwindProtect
          (foreach r (sort (mapcar (lambda (x) (SfyAtomStr x)) (car (errset (results)))) nil)
            (when (and (car (errset (selectResult (stringToSymbol r))))
                       (not (and (SfyResSidebands port r) (setq any t))))
              (fprintf port "# analysis %s\n" r)
              (foreach o (sort (mapcar (lambda (x) (SfyAtomStr x)) (car (errset (outputs)))) nil)
                ;; A net is a voltage; what v() does not answer, a terminal, is a current.
                (setq kind "V")
                (setq w (car (errset (v o))))
                (unless (SfyResIsWave w)
                  (setq kind "I")
                  (setq w (car (errset (i o)))))
                (cond
                  ((SfyResIsWave w)
                    (SfyResWave port o w kind)
                    (setq any t))
                  ;; The operating point: a node voltage, else a terminal current, as one
                  ;; point (live: v("/out") answers 0.5 in dcOp, a terminal only to i()).
                  ((and (equal r "dcOp")
                        (or (SfyResIsNum (setq w (car (errset (v o)))))
                            (and (setq kind "I") (SfyResIsNum (setq w (car (errset (i o)))))))
                        (not (complexp w)))
                    (fprintf port "# %s 1 points %s\n0 %.17g\n" o
                      (if (SfyResIsNum (car (errset (v o)))) "V" "I") (float w))
                    (setq any t))
                  ;; A device's operating-point parameters (live: v("/R0")~>?? answers
                  ;; ("trise" nan "v" 0.5 "i" 0.0005 ...)), one point each as "<inst>:<param>";
                  ;; Rust names them as ngspice's @<dev>[<param>] through the bench.
                  ((and (equal r "dcOpInfo") (setq w (car (errset (getSGq (v o) ??)))) (listp w))
                    (while (cdr w)
                      (when (and (stringp (car w)) (numberp (cadr w)))
                        (fprintf port "# %s:%s 1 points V\n0 %.17g\n" o (car w) (float (cadr w)))
                        (setq any t))
                      (setq w (cddr w))))
                  ;; Anything else with no waveform (operating-point parameters, noise
                  ;; contributions and pac sidebands as families, stb margins), but the
                  ;; netlist's own info results: named so the import reports it.
                  ((not (member r (list "model" "instance" "output" "designParamVals"
                                    "primitives" "subckts" "variables")))
                    (fprintf port "# skipped %s %s\n" r o))))))
          (close port))))
    any))

;; When the selected result r (but noise, whose families are per-device contributions) is a
;; family over sidebands (live pac: famIsFamily, sweepValues -7 to 7), writes one "# analysis
;; r" block per sideband with every output's waveform there (consecutive plots of the same
;; signals are steps in Schemify, waves.md section 1) after a "# steps r <values>" line, and
;; answers t; else nil, writing nothing.
(procedure (SfyResSidebands port r)
  (let ((outs (sort (mapcar (lambda (x) (SfyAtomStr x)) (car (errset (outputs)))) nil))
        (w nil) (sbs nil) (fw nil) (kind nil) (x0 nil))
    ;; The first output v() or i() answers (a terminal answers only i(), live).
    (when (nequal r "noise")
      (foreach o outs
        (unless w (setq w (or (car (errset (v o))) (car (errset (i o))))))))
    (when (and w (car (errset (famIsFamily w))))
      (setq sbs (car (errset (sweepValues w)))))
    ;; Every member on the sweep's own x (the member of sideband 0, else the first): a
    ;; sideband's own x is its output frequency |k fund + f|, descending for k < 0 (live), so
    ;; steps of one x keep the sweep's meaning.
    (when sbs
      (setq x0 (car (errset (drGetWaveformXVec
                              (famValue w (or (car (setof sb sbs (equal sb 0))) (car sbs))))))))
    (when (and sbs x0)
      (fprintf port "# steps %s" r)
      (foreach sb sbs (fprintf port " %s" (SfyAtomStr sb)))
      (fprintf port "\n")
      (foreach sb sbs
        (fprintf port "# analysis %s\n" r)
        (foreach o outs
          (setq kind "V")
          (setq w (car (errset (v o))))
          (unless w
            (setq kind "I")
            (setq w (car (errset (i o)))))
          (setq fw (and w (car (errset (famValue w sb)))))
          (if (SfyResIsWave fw)
            (SfyResWaveOn port o fw kind x0)
            (fprintf port "# skipped %s %s\n" r o))))
      t)))

;; Whether w is a waveform of numbers: its x and y vectors answer, and their first elements
;; are numbers (the primitives result's vectors hold strings, "model").
(procedure (SfyResIsWave w)
  (and w
    (SfyResIsNum (car (errset (drGetElem (drGetWaveformXVec w) 0))))
    (SfyResIsNum (car (errset (drGetElem (drGetWaveformYVec w) 0))))))

;; Whether e is a number, real or complex.
(procedure (SfyResIsNum e)
  (or (numberp e) (complexp e)))

;; SfyResWave with the x values of vector x (as long as w's).
(procedure (SfyResWaveOn port name w kind x)
  (SfyResWaveXY port name x (drGetWaveformYVec w) kind))

;; Prints waveform w of signal name: its header and every point at %.17g.
(procedure (SfyResWave port name w kind)
  (SfyResWaveXY port name (drGetWaveformXVec w) (drGetWaveformYVec w) kind))

(procedure (SfyResWaveXY port name x y kind)
  (let ((n 0) (e nil))
    (setq n (drVectorLength x))
    (fprintf port "# %s %d points %s\n" name n kind)
    (for k 0 (sub1 n)
      (setq e (drGetElem y k))
      (if (complexp e)
        (fprintf port "%.17g %.17g %.17g\n" (float (drGetElem x k)) (float (real e))
          (float (imag e)))
        (fprintf port "%.17g %.17g\n" (float (drGetElem x k)) (float e))))))
