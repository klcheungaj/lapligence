// RTL-018 A01 configuration (SV 2009 §§33.4.1, 33.4.3). The default liblist
// keeps `work` so the interfaces and tap resolve from the parent library.
config rtl018_cfg;
  design work.tb;
  default liblist work rtl gate;
  instance tb use #(.SCALE(3));
  instance tb.alt.u use gate.rtl018_lane #(.GAIN(6));
endconfig
