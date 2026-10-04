// SIM-004 negative: chandles shall not be used in continuous assignments
// (IEEE 1800-2009 6.14), including as a member of a continuously assigned
// record.
module tb;
  typedef struct {string s; chandle h;} rec_t;
  rec_t ra, rb;
  assign rb = ra;
  initial $finish(0);
endmodule
