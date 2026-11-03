module tb;
  initial begin
    $vpi_say(1);
    $display("display 1");
    $vpi_say(2);
    $finish;
  end
endmodule
