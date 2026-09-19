module child #(parameter W = 4)(output [W-1:0] value);
  assign value = {W{1'b1}};
endmodule
module tb;
  wire [3:0] narrow;
  wire [7:0] changed, explicit_override;
  child original(narrow);
  child by_defparam(changed);
  child #(.W(8)) by_parameter(explicit_override);
  defparam by_defparam.W = 8;
  initial begin
    #1;
    $display("n=%h def=%h param=%h DW=%0d PW=%0d DB=%0d PB=%0d", narrow, changed, explicit_override, by_defparam.W, by_parameter.W, $bits(by_defparam.value), $bits(by_parameter.value));
    $finish(0);
  end
endmodule
