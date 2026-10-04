// RTL-018 A02 negative: a liblist without the parent library cannot bind a
// cell that exists only in `work` (SV 2009 §33.4.1.5).
config rtl018_liblist_cfg;
  design work.tb;
  default liblist gate;
endconfig
