// SIM-009-A03: waited `ref` formals bound to fixed-array elements (static,
// multidimensional, real, hierarchical and task-local), a class property and
// a record-carrying task run on the typed stackless call path, so each task
// recurses (SV 13.3.1, 13.5.2, 9.4.2, 9.4.3).
module sub;
  int cells[3];
endmodule

module tb;
  typedef struct { string tag; int q[$]; } rec_t;

  class C;
    int x;
    task automatic climb(ref int r, input int n);
      wait (r >= n);
      $display("climb %0d at %0d", n, $time);
      if (n < 3) climb(r, n + 1);
    endtask
  endclass

  sub u();
  int arr[4];
  logic [1:0] grid [2][3];
  real rs[2];
  C c;
  rec_t rec;

  task automatic count(ref int r, input int n);
    wait (r >= n);
    $display("count %0d at %0d", n, $time);
    if (n < 4) count(r, n + 1);
  endtask

  task automatic edges(ref logic [1:0] r, input int n);
    @(posedge r[0]);
    $display("edge %0d %0d at %0d", n, r, $time);
    if (n > 0) edges(r, n - 1);
  endtask

  task automatic reals(ref real r, input int n);
    @(r);
    $display("real %0d %0.1f at %0d", n, r, $time);
    if (n > 0) reals(r, n - 1);
  endtask

  task automatic carried(ref int r, input rec_t x, input int n);
    wait (r == n);
    $display("%s %0d %0d at %0d", x.tag, x.q.size(), n, $time);
    x.q.push_back(n);
    if (n < 2) carried(r, x, n + 1);
  endtask

  task automatic level(ref int r, input int v);
    wait (r == v);
    $display("local %0d at %0d", v, $time);
  endtask

  task automatic outer(input int n);
    int la[2];
    la[1] = n;
    level(la[1], n);
    if (n > 0) outer(n - 1);
  endtask

  initial begin
    c = new;
    rec.tag = "a";
    rec.q.push_back(7);
    fork
      count(arr[2], 1);
      edges(grid[1][2], 1);
      reals(rs[1], 1);
      carried(u.cells[0], rec, 0);
      c.climb(c.x, 1);
    join_none
    #1 arr[2] = 1;
    #1 arr[1] = 9;
    #1 arr[2] = 3;
    #1 arr[2] = 4;
    #1 grid[1][2] = 2'b00;
    #1 grid[1][2] = 2'b01;
    #1 grid[1][1] = 2'b01;
    #1 grid[1][2] = 2'b10;
    #1 grid[1][2] = 2'b11;
    #1 rs[1] = 1.5;
    #1 rs[0] = 2.0;
    #1 rs[1] = 2.5;
    #1 u.cells[0] = 1;
    #1 u.cells[0] = 2;
    #1 c.x = 1;
    #1 c.x = 3;
    #1 outer(2);
    #1 $finish;
  end
endmodule
