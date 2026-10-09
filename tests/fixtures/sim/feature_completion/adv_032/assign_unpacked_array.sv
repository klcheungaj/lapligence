module tb;
  logic [7:0] words [0:1];
  initial begin
    assign words = '{8'h1, 8'h2};
    #1 $display("%h %h", words[0], words[1]);
    $finish;
  end
endmodule
