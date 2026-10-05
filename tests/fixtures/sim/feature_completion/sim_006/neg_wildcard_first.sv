module tb;
  int w[*];
  int k;
  initial begin
    w[1] = 2;
    void'(w.first(k));
  end
endmodule
