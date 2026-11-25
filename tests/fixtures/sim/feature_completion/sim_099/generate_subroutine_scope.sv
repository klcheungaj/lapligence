// `%m` in a subroutine that a generate block declares (IEEE 1800-2009
// 21.2.1.6, 27.4): the block's name, once per loop iteration, then the
// subroutine and the named blocks in it, also below instances in a generate
// loop and in an array of instances. readme.md derives every line.
`timescale 1ns/1ns

interface bus;
  if (1) begin : ig
    function automatic void show(); $display("interface %m"); endfunction
  end
endinterface

module leaf #(int AT = 0);
  if (1) begin : l
    task st; begin : show $display("static task %m"); end endtask
    task automatic at; begin : show $display("automatic task %m"); end endtask
    function int sf(int x);
      begin : fb $display("static function %m"); end
      sf = x;
    endfunction
    function automatic int af(int x); $display("automatic function %m"); af = x; endfunction
    task automatic rec(int n);
      if (n > 0) rec(n - 1);
      else fork : fk #1 $display("recursive task %m"); join
    endtask
  end
  for (genvar i = 0; i < 2; i++) begin : lp
    task t; $display("loop task %m"); endtask
    if (i == 1) begin : cond
      task t; $display("conditional task %m"); endtask
    end
    case (i)
      0: begin
        task t; $display("case task %m"); endtask
        initial #(AT + 1) t;
      end
    endcase
  end
  for (genvar i = 0; i < 2; i++) begin
    task t; $display("unnamed loop task %m"); endtask
    initial #(AT + 2 + i) t;
  end
  task t; $display("module task %m"); endtask
  task all;
    int x;
    l.st; l.at; x = l.sf(1); x = l.af(2); l.rec(2);
    lp[0].t; lp[1].t; lp[1].cond.t; t;
  endtask
  initial #(AT + 10) all;
endmodule

module tb;
  for (genvar k = 0; k < 2; k++) begin : g
    if (k == 0) begin : q
      leaf #(100) u();
    end else leaf #(200) u();
    bus b();
  end
  leaf #(300) one();
  leaf #(400) arr[1:1]();
  initial begin
    #500 g[0].b.ig.show();
    g[1].b.ig.show();
    $finish(0);
  end
endmodule
