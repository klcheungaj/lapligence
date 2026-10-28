// ADV-019 family 5: the VPI application dynamic.c walks the string, chandle
// and class variables, the class objects they refer to, the class
// definitions and typespecs, and the dynamic arrays, queues and associative
// arrays of this module (IEEE 1800-2009 37.17, 37.27-37.29), then writes
// values the design prints. See readme.md for the expected output.
module tb;
  class Packet;
    int id;
    string name;
    real weight;
    Packet next;
    byte data[$];
    function new(int i);
      id = i;
      name = "pkt";
      weight = 1.5;
    endfunction
  endclass
  class LinkedPacket extends Packet;
    logic [7:0] tag;
    function new(int i);
      super.new(i);
      tag = 8'h5a;
    endfunction
  endclass
  string s = "hi";
  chandle ch = null;
  Packet p;
  Packet none = null;
  Packet ring[$];
  int da[];
  byte q[$];
  int aa[int];
  string sa[string];
  initial begin
    LinkedPacket l;
    l = new(3);
    p = l;
    p.next = new(4);
    p.data.push_back(8'd7);
    ring.push_back(p);
    ring.push_back(null);
    da = new[3];
    da[1] = 11;
    q.push_back(1);
    q.push_back(2);
    aa[5] = 7;
    aa[-2] = 9;
    sa["k"] = "v";
    $walk();
    $display("s=%s id=%0d da[2]=%0d q[0]=%0d aa[5]=%0d sa[k]=%s tag=%h next.id=%0d",
             s, p.id, da[2], q[0], aa[5], sa["k"], l.tag, p.next.id);
    $display("name=%s weight=%0.1f ch=%0d none=%0d", p.name, p.weight, ch == null,
             none == null);
    q.delete(0);
    $stale();
  end
endmodule
