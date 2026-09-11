module tb;
    int a;
    initial a<=#7 42;
    final $display("final %0t %0d",$time,a);
endmodule
