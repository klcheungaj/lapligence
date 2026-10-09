module tb;
  class Box;
    int value;
  endclass
  Box handle;
  initial begin
    assign handle = null;
    #1 $display("%0d", handle == null);
    $finish;
  end
endmodule
