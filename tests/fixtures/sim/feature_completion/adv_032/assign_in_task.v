module tb;
  reg [3:0] v;
  task hold_value;
    begin
      assign v = 4'h5;
    end
  endtask
  initial begin
    hold_value;
    #1 $display("%h", v);
    $finish;
  end
endmodule
