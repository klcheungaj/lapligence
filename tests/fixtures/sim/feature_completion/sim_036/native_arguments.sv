// SIM-036 A02: action arguments passed by value keep their values from the
// instant the deferred assertion executed, for native and packed types;
// ref and const ref arguments read static variables in the Reactive region
// (IEEE 1800-2009 16.4). A method call's receiver handle is taken when the
// assertion executes (decision S36-D3).
module tb;
  typedef struct packed { logic [3:0] h; logic [3:0] l; } nibbles_t;
  typedef struct { int a; logic [7:0] b; } pair_t;

  class Box;
    int v;
    string name;
    function new(string n, int value);
      name = n;
      v = value;
    endfunction
    function void show(input int a);
      $display("%0d %s show a=%0d", $time, name, a);
    endfunction
  endclass

  string s;
  chandle p;
  real r;
  nibbles_t n;
  pair_t pr;
  Box bx;
  logic [99:0] wide;
  int arr [3];
  int live;

  task strings(input string x, input string y);
    $display("%0d x=%s y=%s now=%s", $time, x, y, s);
  endtask

  task handle(input chandle h);
    $display("%0d chandle null=%0d", $time, h == null);
  endtask

  task values(input real xr, input nibbles_t xn, input pair_t xp, input Box xb,
              input logic [99:0] xw);
    $display("%0d r=%0.2f n=%h pair=%0d/%0d box=%s/%0d w=%h", $time, xr, xn,
             xp.a, xp.b, xb.name, xb.v, xw);
  endtask

  task array(input int xa [3]);
    $display("%0d arr=%0d %0d %0d", $time, xa[0], xa[1], xa[2]);
  endtask

  task automatic refs(ref int x, const ref string y);
    $display("%0d ref x=%0d y=%s", $time, x, y);
  endtask

  initial begin
    s = "issue";
    p = null;
    r = 1.5;
    n = 8'h5a;
    pr.a = 3;
    pr.b = 8'd4;
    bx = new("first", 7);
    wide = {4'hf, 96'd1};
    arr = '{1, 2, 3};
    live = 1;
    assert #0 (1'b0) else strings(s, {s, "!"});
    assert #0 (1'b0) else handle(p);
    assert #0 (1'b0) else values(r, n, pr, bx, wide);
    assert #0 (1'b0) else array(arr);
    assert #0 (1'b0) else refs(live, s);
    assert #0 (1'b0) else bx.show(live);
    // Mutate everything after issue, in the same time step.
    s = "later";
    r = 2.5;
    n = 8'h00;
    pr.a = 30;
    bx = new("second", 70);
    wide = '0;
    arr = '{9, 9, 9};
    live = 2;
    #1 $finish(0);
  end
endmodule
