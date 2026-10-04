// IEEE 1800-2009 7.12.2: unpacked records have no relational order, so
// sort needs an integral with key.
module tb;
  typedef struct {
    int k;
    byte v;
  } rec_t;
  rec_t r [0:19];
  initial begin
    r.sort();
    $finish;
  end
endmodule
