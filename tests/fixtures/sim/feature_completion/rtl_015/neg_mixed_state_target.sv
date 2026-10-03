// IEEE 1800-2009 6.24.3, 11.4.14.4: elements mixing two-state and four-state
// members need member-wise conversion, which a runtime-selected packed element
// write of a record member array does not provide; it is rejected.
module tb;
  typedef struct { bit [3:0] a; logic [3:0] b; } mix_t;
  typedef struct { logic [3:0] h; mix_t cells [0:2]; } rec_t;
  rec_t r;
  int i;
  initial begin
    i = 1;
    {>>{r.cells with [i +: 2]}} = 16'hABCD;
    $finish;
  end
endmodule
