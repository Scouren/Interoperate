;; a_probe2.il: lane A (gaps round 2, round 4) research probe for the audit's open ADE rows
;; (gaps2/FINAL.md): environment options includePath/stimulusFile, the save set and the
;; Spectre APS/X mode (row 5, 17); xf and hb results (row 21); a 3-sample Monte Carlo history
;; (row 22). In chkLib (sfyprobe_a): rc (V0 vsin -> R0 r=1k*rv -> out, C0 and R1 to
;; ground), a model file whose statistics vary rv. Writes out/a_probe2/ (netlists, the
;; histories' dumps through SfyAdeResults) and SfyExport's dump out/a_probe2/cells.sfyx.

chkW = getShellEnvVar("SFYCHECK_WORK")
chkOut = strcat(chkW "/out/a_probe2")
chkSim = strcat(chkW "/sim_ap2")
system(sprintf(nil "rm -rf '%s' '%s'; mkdir -p '%s'" chkOut chkSim chkOut))
envSetVal("asimenv.startup" "projectDir" 'string chkSim)

procedure(chkSet(inst name value)
  let((c)
    c = cdfFindParamByName(cdfGetInstCDF(inst) name)
    if(c then c~>value = value t else chkLine(sprintf(nil "no CDF param %s on %s" name inst~>name) nil))))

procedure(chkStub(cv inst nets)
  let((bb c p2 w ib dx)
    ib = inst~>bBox
    foreach(term inst~>master~>terminals
      when(assoc(term~>name nets)
        bb = dbTransformBBox(car(term~>pins)~>fig~>bBox inst~>transform)
        c = list((xCoord(car(bb)) + xCoord(cadr(bb))) / 2.0 (yCoord(car(bb)) + yCoord(cadr(bb))) / 2.0)
        dx = if(xCoord(c) < (xCoord(car(ib)) + xCoord(cadr(ib))) / 2.0 - 0.01 then -0.25 else 0.25)
        p2 = list(xCoord(c) + dx yCoord(c))
        w = schCreateWire(cv "draw" "full" list(c p2) 0.0625 0.0625 0.0)
        schCreateWireLabel(cv car(w) p2 cadr(assoc(term~>name nets)) "lowerLeft" "R0" "stick" 0.0625 nil)))))

let((cv v r c r1 port)
  cv = dbOpenCellViewByType(chkLib "rc" "schematic" "schematic" "w")
  v = schCreateInst(cv dbOpenCellViewByType("analogLib" "vsin" "symbol") "V0" 0:0 "R0")
  r = schCreateInst(cv dbOpenCellViewByType("analogLib" "res" "symbol") "R0" 6:0 "R0")
  c = schCreateInst(cv dbOpenCellViewByType("analogLib" "cap" "symbol") "C0" 9:0 "R0")
  r1 = schCreateInst(cv dbOpenCellViewByType("analogLib" "res" "symbol") "R1" 12:0 "R0")
  foreach(kv '(("vdc" "1") ("va" "0.1") ("freq" "1M") ("acm" "1")) chkSet(v car(kv) cadr(kv)))
  chkSet(r "r" "1k*rv") chkSet(r1 "r" "1k") chkSet(c "c" "1n")
  chkStub(cv v '(("PLUS" "in") ("MINUS" "gnd!")))
  chkStub(cv r '(("PLUS" "in") ("MINUS" "out")))
  chkStub(cv c '(("PLUS" "out") ("MINUS" "gnd!")))
  chkStub(cv r1 '(("PLUS" "out") ("MINUS" "gnd!")))
  chkE("rc check" lambda(() schCheck(cv)))
  dbSave(cv) dbClose(cv)
  port = outfile(strcat(chkOut "/stat.scs") "w")
  fprintf(port "simulator lang=spectre\nstatistics {\n  process {\n    vary rv dist=gauss std=0.1\n  }\n}\n")
  close(port))

chkS = chkE("open maestro" list('maeOpenSetup chkLib "rc" "maestro" ?mode "a"))
chkSdb = chkE("sdb" lambda(() axlGetMainSetupDB(chkS)))
;; rv: the design variable ADE makes of R0's r (a parameters line of rv in the model
;; file beside it is SFE-59, live).
chkE("var rv" lambda(() maeSetVar("rv" "1" ?session chkS)))
chkE("TX" lambda(() maeCreateTest("TX" ?lib chkLib ?cell "rc" ?view "schematic" ?simulator "spectre" ?session chkS)))
errset(maeSetAnalysis("TX" "tran" ?enable nil ?session chkS))
chkE("TX xf" lambda(() maeSetAnalysis("TX" "xf" ?enable t ?options '(("start" "1k") ("stop" "100k") ("incrType" "Logarithmic")
  ("stepTypeLog" "Points Per Decade") ("dec" "2") ("outType" "voltage") ("p" "/out") ("n" "/gnd!")) ?session chkS)))
chkE("TX models" lambda(() maeSetEnvOption("TX" ?options list(list("modelFiles" list(list(strcat(chkOut "/stat.scs") "")))) ?session chkS)))
;; rows 5, 17: environment options and the simulator mode
foreach(o '("includePath" "stimulusFile" "uniMode" "turboSwitch" "save" "simExecName" "spectreRunMode" "apsplus")
  chkE(sprintf(nil "TX env %s" o) lambda(() maeGetEnvOption("TX" ?option o ?session chkS)))
  chkE(sprintf(nil "TX sim %s" o) lambda(() maeGetSimOption("TX" ?option o ?session chkS))))
chkE("TX set includePath" lambda(() maeSetEnvOption("TX" ?options '(("includePath" "/tmp/inc")) ?session chkS)))
chkE("TX set stimulusFile" lambda(() maeSetEnvOption("TX" ?options list(list("stimulusFile" strcat(chkOut "/stim.scs"))) ?session chkS)))
let((port) port = outfile(strcat(chkOut "/stim.scs") "w") fprintf(port "simulator lang=spectre\n// stimuli\n") close(port))
chkE("TX set uniMode APS (env)" lambda(() maeSetEnvOption("TX" ?options '(("uniMode" "APS") ("turboSwitch" t)) ?session chkS)))
chkE("TX set save (sim)" lambda(() maeSetSimOption("TX" ?options '(("save" "selected")) ?session chkS)))
foreach(o '("includePath" "stimulusFile" "uniMode" "turboSwitch" "save")
  chkE(sprintf(nil "TX env after %s" o) lambda(() maeGetEnvOption("TX" ?option o ?session chkS)))
  chkE(sprintf(nil "TX sim after %s" o) lambda(() maeGetSimOption("TX" ?option o ?session chkS))))
chkE("TX out" lambda(() maeAddOutput("vo" "TX" ?outputType "point" ?expr "ymax(mag(VF(\"/out\")))" ?session chkS)))
;; row 22: Monte Carlo, 3 samples
chkE("TM" lambda(() maeCreateTest("TM" ?lib chkLib ?cell "rc" ?view "schematic" ?simulator "spectre" ?session chkS)))
chkE("TM dc" lambda(() maeSetAnalysis("TM" "dc" ?enable t ?options '(("saveOppoint" t)) ?session chkS)))
errset(maeSetAnalysis("TM" "tran" ?enable nil ?session chkS))
chkE("TM models" lambda(() maeSetEnvOption("TM" ?options list(list("modelFiles" list(list(strcat(chkOut "/stat.scs") "")))) ?session chkS)))
chkE("TM out" lambda(() maeAddOutput("vout" "TM" ?outputType "point" ?expr "VDC(\"/out\")" ?session chkS)))
foreach(kv '(("mcnumpoints" "3") ("mcmethod" "process") ("donominal" "0"))
  chkE(sprintf(nil "MC %L" kv) lambda(() axlSetRunOptionValue(axlPutRunOption(chkSdb "Monte Carlo Sampling" car(kv)) cadr(kv)))))
chkE("save" lambda(() maeSaveSetup(?lib chkLib ?cell "rc" ?view "maestro" ?session chkS)))
let((d)
  d = strcat(chkOut "/nl_TX")
  chkE("netlist TX" lambda(() maeCreateNetlistForCorner("TX" "Nominal" d ?session chkS)))
  system(sprintf(nil "cd '%s/netlist' && grep -E '^(xf|hb|include|ahdl|save|simulatorOptions)|^ +' input.scs > ../lines.txt; cat runSimulation > ../run.txt 2>&1; ls > ../files.txt" d)))
;; one run of TX alone (xf, hb), then the Monte Carlo of TM alone
chkE("TM off" lambda(() maeSetSetup(?tests '("TM") ?enabled nil ?session chkS)))
chkHx = chkE("run single" lambda(() maeRunSimulation(?session chkS)))
chkE("wait" lambda(() maeWaitUntilDone('All)))
chkE("TM on, TX off" lambda(() list(maeSetSetup(?tests '("TM") ?enabled t ?session chkS) maeSetSetup(?tests '("TX") ?enabled nil ?session chkS))))
chkHm = chkE("run MC" lambda(() maeRunSimulation(?runMode "Monte Carlo Sampling" ?session chkS)))
chkE("wait MC" lambda(() maeWaitUntilDone('All)))
chkE("close" lambda(() maeCloseSession(?session chkS ?forceClose t)))
foreach(h list(chkHx chkHm)
  when(stringp(h)
    chkThat(sprintf(nil "SfyAdeResults %s" h) SfyAdeResults(chkOut chkLib "rc" h))))
chkThat("SfyExport rc" SfyExport(strcat(chkOut "/cells.sfyx") chkLib '("rc")))
;; The simulation tree stays for the shell step's logs when SFYCHECK_KEEP_SIM is set.
unless(getShellEnvVar("SFYCHECK_KEEP_SIM") system(sprintf(nil "rm -rf '%s'" chkSim)))
