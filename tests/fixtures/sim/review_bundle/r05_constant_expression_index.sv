// IEEE 1800-2009 10.9: array keys accept constant expressions, not just digits.
module tb;
  localparam int BASE = 0;
  int values[0:1];
  int negative_values[-1:0];
  int function_values[0:1];
  int seed;
  function automatic int plus_one(input int value);
    plus_one = value + 1;
  endfunction
  initial begin
    seed = 23;
    values = '{(BASE+1):seed, default:0};
    negative_values = '{(-1):seed, default:0};
    function_values = '{plus_one(BASE):seed, default:0};
    if (values[0] !== 0 || values[1] !== 23 ||
        negative_values[-1] !== 23 || negative_values[0] !== 0 ||
        function_values[0] !== 0 || function_values[1] !== 23)
      $fatal(1, "constant index key");
    $display("PASS r05_constant_expression_index");
    $finish;
  end
endmodule
