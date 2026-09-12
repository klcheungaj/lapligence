// llg-test-fixture: tests/fixtures/sim/partial_features/severity_fatal.sv
module tb;
  integer evals = 0;

  function integer bump(input integer value);
    begin
      evals = evals + 1;
      bump = value;
    end
  endfunction

  initial begin
    $display("stdout before");
    $fatal(0, "fatal=%0d", bump(7));
    $display("stdout after");
  end

  final begin
    $display("stdout final evals=%0d", evals);
  end
endmodule
