// SIM-009-A03: waited `ref` formals bound to members of unpacked structures
// (a module variable, an array element's member and an automatic) follow the
// member's own bits, so writes to sibling members do not wake them; the
// tasks recurse (SV 9.4.2, 9.4.3, 13.5.2).
module tb;
  typedef struct {
    logic [3:0] a;
    logic [3:0] b;
  } pair_t;
  typedef struct {
    int n;
    pair_t p;
  } outer_t;
  pair_t us;
  outer_t arr[2];

  task automatic change(ref logic [3:0] r, input int n);
    @(r);
    $display("change %0d %0d at %0d", n, r, $time);
    if (n > 0) change(r, n - 1);
  endtask

  task automatic reach(ref int r, input int v);
    wait (r == v);
    $display("reach %0d at %0d", v, $time);
  endtask

  task automatic local_member();
    pair_t lp;
    lp.a = 0;
    lp.b = 0;
    fork
      change(lp.a, 0);
      begin
        #1 lp.b = 2;
        #1 lp.a = 6;
      end
    join
  endtask

  initial begin
    us.a = 0;
    us.b = 0;
    arr[1].n = 0;
    arr[1].p.b = 0;
    fork
      change(us.b, 1);
      reach(arr[1].n, 3);
      change(arr[1].p.b, 0);
    join_none
    #1 us.a = 3;
    #1 us.b = 4;
    #1 us.b = 5;
    #1 arr[0].n = 3;
    #1 arr[1].p.a = 1;
    #1 arr[1].n = 3;
    #1 arr[1].p.b = 9;
    #1 local_member();
    $finish;
  end
endmodule
