// Legal in both supplied editions: concatenation is unsigned, even with one part.
module tb;
  reg signed [3:0] value;
  reg [31:0] result;
  function [31:0] widen;
    input signed [3:0] argument;
    begin
      widen = {argument};
    end
  endfunction
  function [31:0] direct;
    input signed [3:0] argument;
    begin
      direct = argument;
    end
  endfunction
  initial begin
    value = -4'sd2;
    result = widen(value);
    if (result !== 32'd14 || direct(value) !== 32'hfffffffe) begin
      $display("FAIL n06_singleton_concat_legacy");
      $finish(0);
    end
    value = 4'bz010;
    result = widen(value);
    if (result !== {28'b0, 4'bz010}) begin
      $display("FAIL n06_singleton_concat_legacy");
      $finish(0);
    end
    $display("PASS n06_singleton_concat_legacy");
    $finish(0);
  end
endmodule
