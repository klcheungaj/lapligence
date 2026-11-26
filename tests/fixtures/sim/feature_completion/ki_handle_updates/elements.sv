// Event-controlled assignments to elements, slices and whole arrays of handles
// and strings (readme: "Event controls").
`timescale 1ns / 1ns
package pk;
  class node;
    int m;
    function new(int m);
      this.m = m;
    endfunction
  endclass
  node pa[2];
endpackage
interface bus;
  pk::node ia[2];
endinterface
module tb;
  import pk::*;
  class st;
    static node sa[2];
    static node sb[2][2];
  endclass
  bus b ();
  node ha[3], h2[2][3], n1 = new(1), n2 = new(2);
  event ev;
  logic clk = 0;
  int i, j;
  function automatic int g(node x);
    return x == null ? -1 : x.m;
  endfunction
  function automatic node mk(int x);
    node made = new(x);
    return made;
  endfunction
  function automatic int ix(int x);
    $display("index %0d", x);
    return x;
  endfunction
  task show(string step);
    $display("%s: ha %0d %0d %0d h2 %0d %0d pa %0d ia %0d sa %0d sb %0d", step, g(ha[0]), g(ha[1]),
             g(ha[2]), g(h2[1][2]), g(h2[0][0]), g(pa[1]), g(b.ia[1]), g(st::sa[1]),
             g(st::sb[1][0]));
  endtask
  initial begin
    i = 1;
    j = 2;
    ha[i] <= @(ev) n1;
    ha[ix(2)] <= @(ev) mk(7);
    h2[i][j] <= @(posedge clk) mk(8);
    h2[0][0] <= repeat (2) @(ev) mk(9);
    pa[i] <= @(ev) n2;
    b.ia[i] <= @(ev) mk(10);
    st::sa[i] <= @(ev) mk(11);
    st::sb[i][0] <= @(ev) mk(12);
    ha[i+5] <= @(ev) n2;
    i = 0;
    j = 0;
    n1 = n2;
    show("issued");
    #1 ->ev;
    show("triggered");
    #1 show("after");
    ->ev;
    clk = 1;
    #1 show("second");
    ha <= @(ev) '{n2, null, n2};
    h2[0] <= @(ev) '{n2, n2, null};
    #1 ->ev;
    #1 show("whole");
    $display("%0d", g(h2[0][1]));
    $finish(0);
  end
endmodule
