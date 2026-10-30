// SIM-046 statement coverage (decision S46-D13) leaves the behaviour of the
// code it counts unchanged: a unique-if ladder still reports two true
// conditions (12.4.2), a named block forming a whole fork branch still ends
// the branch KILLED when disabled (9.6.2, 9.7), the function that a
// continuous assignment and a $strobe call stays a helper with private
// loops and locals, and the automatic variable of a process without other
// effects stays a frame cell. Derivations: readme.md.
module tb;
  logic first = 1'b1;
  logic second = 1'b1;
  logic [7:0] x = 8'd0;
  wire [7:0] count;
  process whole;

  function automatic logic [7:0] ones(input logic [7:0] v);
    logic [7:0] n = 8'd0;
    for (int i = 0; i < 8; i++)
      if (v[i]) n++;
    return n;
  endfunction
  assign count = ones(x);

  initial begin
    automatic integer left = 20;
    unique if (first) $display("first %0d", left);
    else if (second) $display("second");
    fork
      begin : whole_blk
        whole = process::self();
        #10 $display("unexpected");
      end
    join_none
    #1 disable whole_blk;
    x = 8'b1011_0001;
    $strobe("strobe count %0d", ones(x));
    #1 $display("whole: %s count %0d left %0d", whole.status().name(), count, left);
    $display("statements %0d/%0d", $coverage_get(`SV_COV_STATEMENT, `SV_COV_MODULE, tb),
             $coverage_get_max(`SV_COV_STATEMENT, `SV_COV_MODULE, tb));
    $finish;
  end

  initial begin
    automatic integer right = 22;
    #1 $display("right %0d", right);
  end
endmodule
