interface bus;
  wand value;
endinterface
module tb;
  bus b();
  assign b.value = 1'b1;
  initial begin
    #1 $display("value=%b", b.value);
    $finish(0);
  end
endmodule
