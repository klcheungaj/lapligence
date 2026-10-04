// SIM-004 negative: ports shall not have the chandle data type, including
// value links of a record that contains a chandle (IEEE 1800-2009 6.14).
typedef struct {chandle h; string s;} rec_t;
module child(input rec_t value);
endmodule
module tb;
  rec_t value;
  child c(value);
  initial $finish(0);
endmodule
