// A queue method of a static property reached through a virtual interface is
// rejected (readme: "Object expressions").
interface bus;
  class st;
    static int q[$];
  endclass
  st o = new;
endinterface
module tb;
  bus b ();
  virtual bus v = b;
  initial v.o.q.push_back(1);
endmodule
