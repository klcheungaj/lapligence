// SIM-023 A03: forces outlive the process that applied them; killing or
// disabling that process, and ending the run, leave no dangling force driver.
module tb;
  logic [3:0] d, v, w2, h, e;
  wire [3:0] n;
  assign n = e;
  int calls;
  function automatic logic [3:0] bump(input logic [3:0] x);
    calls++;
    return x + 4'h1;
  endfunction
  task automatic hold_force(input int t);
    force h = d;
    #t;
  endtask
  initial begin
    d = 4'h1; e = 4'h2; calls = 0;
    fork : blk
      begin
        force v = d;
        force n = d + 4'h1;
        #10 release v;
      end
    join_none
    #1 $display("1 v=%h n=%h", v, n);
    disable blk;
    d = 4'h3;
    #1 $display("2 v=%h n=%h", v, n);
    fork
      begin
        force w2 = bump(d);
        #5;
      end
    join_none
    #1 d = 4'h4;
    #1 $display("3 w2=%h", w2);
    disable fork;
    d = 4'h5;
    #1 $display("4 w2=%h", w2);
    release w2;
    d = 4'h6;
    #1 $display("5 w2=%h v=%h n=%h", w2, v, n);
    fork
      hold_force(3);
    join_none
    #1 disable fork;
    d = 4'h7;
    #1 $display("6 h=%h v=%h n=%h", h, v, n);
    $finish;
  end
endmodule
