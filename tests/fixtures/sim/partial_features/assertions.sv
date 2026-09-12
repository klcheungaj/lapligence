// llg-test-fixture: tests/fixtures/sim/partial_features/assertions.sv
module tb;
  integer evals = 0;

  function logic condition(input logic value);
    begin
      evals = evals + 1;
      condition = value;
    end
  endfunction

  initial begin
    assert (condition(1'b1)) $display("assert pass");
    named_assert: assert (condition(1'b0))
      else $display("assert fail");
    assume (condition(1'bx))
      else $display("assume fail");
    cover (condition(1'b1)) $display("cover pass");
    cover (condition(1'bz));
    $display("evals=%0d", evals);
    $finish(2);
  end
endmodule
