// SIM-023 A01: forced concatenations of variables, replacement and release.
module tb;
  reg [3:0] a, b, c, s;
  initial begin
    a = 4'h1; b = 4'h2; c = 4'h3; s = 4'h1;
    force {a, b} = {s, ~s};
    #1 $display("1 a=%h b=%h c=%h", a, b, c);
    s = 4'h3;
    #1 $display("2 a=%h b=%h c=%h", a, b, c);
    force {b, c} = 8'h5c;
    #1 $display("3 a=%h b=%h c=%h", a, b, c);
    s = 4'h7;
    #1 $display("4 a=%h b=%h c=%h", a, b, c);
    release a;
    s = 4'h0;
    #1 $display("5 a=%h b=%h c=%h", a, b, c);
    a = 4'h9; b = 4'h9;
    #1 $display("6 a=%h b=%h c=%h", a, b, c);
    release {b, c};
    b = 4'h0; c = 4'h0;
    #1 $display("7 a=%h b=%h c=%h", a, b, c);
    release a;
    release a;
    $display("8 a=%h", a);
    $finish;
  end
endmodule
