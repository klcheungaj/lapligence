// A ref formal of a record with a string member aliases its actual: a reader
// sees the writes of a suspended task. Rebuild with N = 8 for per-cell leaves.
`ifndef N
`define N 16
`endif
module tb;
  typedef struct { string name; logic [7:0] a[`N]; int s; } rec_t;
  rec_t r, u;
  task automatic bump(ref rec_t x);
    x.a[2] = x.a[2] + 8'h01;
    x.name = {x.name, "+"};
    #1 x.s = x.s + 1;
  endtask
  task automatic outer(ref rec_t x);
    bump(x);
    x.a[3] = 8'h05;
  endtask
  function automatic int len(const ref rec_t x);
    return x.name.len() + x.s;
  endfunction
  task automatic local_ref();
    rec_t l;
    l.name = "l";
    l.a[2] = 8'h10;
    l.s = 0;
    bump(l);
    $display("%s %h %0d", l.name, l.a[2], l.s);
  endtask
  initial begin
    r.name = "a"; r.a[2] = 8'h20; r.s = 1;
    u.name = "u"; u.a[2] = 8'h40; u.s = 10;
    fork
      outer(r);
      begin
        #0 $display("%s %h", r.name, r.a[2]);
      end
    join
    bump(u);
    $display("%s %h %0d %h", r.name, r.a[2], r.s, r.a[3]);
    $display("%s %h %0d %0d", u.name, u.a[2], u.s, len(u));
    local_ref();
    $finish(0);
  end
endmodule
