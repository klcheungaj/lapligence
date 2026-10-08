// SIM-017: record messages with string, real, queue and class-handle members
// through typed unbounded and bounded and untyped mailboxes. The original is
// changed after put: value members were copied, the class handle is shared
// (SV 15.4, 7.2, 7.10, 8.4).
class node_c;
  int v;
  function new(int v);
    this.v = v;
  endfunction
endclass

module tb;
  typedef struct {
    int id;
    string name;
    real weight;
    byte data[$];
    node_c link;
  } pkt_t;
  typedef struct {
    pkt_t inner;
    string tag;
  } wrap_t;

  mailbox #(pkt_t) unbounded = new();
  mailbox #(pkt_t) bounded = new(2);
  mailbox any = new();
  pkt_t src, dst;
  wrap_t w_src, w_dst;
  node_c n;

  initial begin
    n = new(10);
    src.id = 1;
    src.name = "alpha";
    src.weight = 0.5;
    src.data = '{8'h11, 8'h22};
    src.link = n;
    unbounded.put(src);
    bounded.put(src);
    any.put(src);
    src.id = 2;
    src.name = "beta";
    src.weight = 2.25;
    src.data[0] = 8'h99;
    src.data.push_back(8'h33);
    n.v = 20;
    unbounded.get(dst);
    $display("unbounded %0d %s %0.2f %0d %h %h %0d same=%0d", dst.id, dst.name,
             dst.weight, dst.data.size(), dst.data[0], dst.data[1], dst.link.v,
             dst.link == n);
    bounded.get(dst);
    $display("bounded %0d %s %0.2f %0d %h", dst.id, dst.name, dst.weight,
             dst.data.size(), dst.data[0]);
    any.get(dst);
    $display("untyped %0d %s %0d", dst.id, dst.name, dst.data.size());
    dst.data[1] = 8'h00;
    $display("independent %h %0d", src.data[1], src.data.size());
    w_src.inner.id = 3;
    w_src.inner.name = src.name;
    w_src.inner.weight = src.weight;
    w_src.inner.data = src.data;
    w_src.tag = "outer";
    any.put(w_src);
    w_src.inner.name = "gamma";
    w_src.tag = "x";
    any.get(w_dst);
    $display("nested %s %s %0d %h %0.2f", w_dst.tag, w_dst.inner.name,
             w_dst.inner.data.size(), w_dst.inner.data[2], w_dst.inner.weight);
    $finish(0);
  end
endmodule
