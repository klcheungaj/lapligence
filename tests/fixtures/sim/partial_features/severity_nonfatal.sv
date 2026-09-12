// llg-test-fixture: tests/fixtures/sim/partial_features/severity_nonfatal.sv
module tb;
  integer evals = 0;

  function integer bump(input integer value);
    begin
      evals = evals + 1;
      bump = value;
    end
  endfunction

  initial begin
    $info("scope=%m info=%0d literal={args}", bump(1));
    $warning("warning=%0d", bump(2));
    $error("error=%0d", bump(3));
    $display("stdout evals=%0d", evals);
    $finish(2);
  end
endmodule
