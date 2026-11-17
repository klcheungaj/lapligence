// A member of an element of an array of records that hold a real, string,
// handle, event or container member is a legal target (SV 10.6.2, 6.4) that
// llg does not support yet (docs/known_issues.md).
typedef struct { int a; real r; } rr_t;
module tb;
  rr_t x[2];
  initial begin
    force x[1].a = 1;
    #1 $display("%0d", x[1].a);
  end
endmodule
