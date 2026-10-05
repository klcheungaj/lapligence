module tb;
  function reg [3:0] inc;
    input [3:0] v;
    inc = v + 1;
  endfunction
endmodule
