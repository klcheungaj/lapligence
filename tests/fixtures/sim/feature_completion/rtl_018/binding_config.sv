// RTL-018 A02 configurations over binding.sv (SV 2009 §33.4).
// rtl018_gate_cfg binds u to the gate cell regardless of search order.
config rtl018_gate_cfg;
  design work.tb;
  default liblist rtl gate work;
  instance tb.u use gate.rtl018_pick;
endconfig
