// IEEE 1800-2009 7.12.2: unpacked records need an integral ordering key.
module tb;
  typedef struct { byte key; bit flag; } record_t;
  record_t values[0:1];
  initial values.sort();
endmodule
