module tb;
  class K;
    int q[$];
  endclass
  K k;
  initial begin
    k = new;
    k.q.push_back(1);
  end
endmodule
