// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/initial_assertion_bodies.sv
// An initial procedure made only of concurrent assertions starts one attempt
// per assertion, whether the assertion is labeled, nested in begin-end blocks
// or shares the procedure with other assertions. The module-level `st` starts
// an attempt on every tick for contrast. See the fixture readme.
module tb;
  logic clk = 1'b0, a = 1'b1, b = 1'b0;

  always #5 clk = ~clk;

  initial begin
    #10 b = 1'b1;
    #10 b = 1'b0;
    #10 $finish;
  end

  st: cover property (@(posedge clk) a) $display("%0d st %m", $time);

  initial begin : ib
    pc: cover property (@(posedge clk) a) begin : pact
      $display("%0d pc %m", $time);
    end
  end

  initial begin
    c1: cover property (@(posedge clk) a) $display("%0d c1 %m", $time);
    cover property (@(posedge clk) a) $display("%0d c2", $time);
    begin : inner
      lab: assert property (@(posedge clk) a ##1 b)
        $display("%0d lab pass %m", $time);
      else
        $display("%0d lab fail %m", $time);
    end
  end
endmodule
