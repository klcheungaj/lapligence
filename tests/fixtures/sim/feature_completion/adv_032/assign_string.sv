module tb;
  string name;
  initial begin
    assign name = "abc";
    #1 $display("%s", name);
    $finish;
  end
endmodule
