// IEEE 1800-2009 6.24.3, 11.4.14.4: formerly the RTL-015 negative
// `neg_mixed_state_target`. A runtime-selected record member array whose
// elements mix two-state and four-state members converts member-wise.
module tb;
  typedef struct { bit [3:0] a; logic [3:0] b; } mix_t;
  typedef struct { logic [3:0] h; mix_t cells [0:2]; } rec_t;
  rec_t r;
  int i;
  initial begin
    i = 1;
    {>>{r.cells with [i +: 2]}} = 16'hABCD;
    $display("%h %h %h %h %h %h", r.cells[0].a, r.cells[0].b, r.cells[1].a, r.cells[1].b,
             r.cells[2].a, r.cells[2].b);
    $finish(0);
  end
endmodule
