// RTL-018 A02 negative: a use clause naming a library that does not exist.
config rtl018_missing_cfg;
  design work.tb;
  default liblist rtl gate work;
  instance tb.u use nolib.rtl018_pick;
endconfig
