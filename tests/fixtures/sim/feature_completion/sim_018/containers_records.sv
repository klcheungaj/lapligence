// SIM-018: cycles through queue properties and record properties (a record
// member naming a second class whose handle closes the cycle) are
// reclaimed, while module queues, dynamic and associative arrays, record
// members and a class-static property keep their objects (SV 8.27, 8.9,
// 7.5, 7.8, 7.10).
typedef class bag_c;
class peer_c;
  int id;
  bag_c back;
  function new(int init);
    id = init;
  endfunction
endclass

typedef struct {
  int k;
  string s;
  peer_c h;
} rec_t;

class bag_c;
  int id;
  bag_c items[$];
  rec_t r;
  static bag_c registry;
  function new(int init);
    id = init;
  endfunction
endclass

module tb;
  bag_c q[$];
  bag_c da[];
  bag_c aa[string];

  task automatic churn(int n);
    for (int i = 0; i < n; i++) begin
      bag_c x, y;
      peer_c px, py;
      x = new(i);
      y = new(i + 1000);
      x.items.push_back(y);
      y.items.push_back(x);
      px = new(i);
      px.back = y;
      x.r.h = px;
      py = new(i);
      py.back = x;
      y.r.h = py;
      x.r.s = "garbage";
      #1;
    end
  endtask

  initial begin
    bag_c head, tail;
    peer_c p;
    head = new(1);
    tail = new(2);
    head.items.push_back(tail);
    p = new(10);
    p.back = head;
    tail.r.h = p;
    p = null;
    tail.r.s = "kept";
    q.push_back(head);
    head = null;
    tail = null;
    da = new[2];
    da[1] = new(3);
    head = new(4);
    p = new(5);
    head.r.h = p;
    p = null;
    aa["k"] = head;
    bag_c::registry = new(6);
    head = new(7);
    head.items.push_back(bag_c::registry);
    bag_c::registry.items.push_back(head);
    head = null;
    churn(40);
    head = q[0];
    tail = head.items[0];
    p = tail.r.h;
    $display("q %0d %0d %0d %0d %s", head.id, tail.id, p.id, p.back.id, tail.r.s);
    $display("da %0d null=%0d", da[1].id, da[0] == null);
    head = aa["k"];
    p = head.r.h;
    $display("aa %0d %0d", head.id, p.id);
    head = bag_c::registry;
    tail = head.items[0];
    $display("static %0d %0d %0d", head.id, tail.id, tail.items[0].id);
    $finish(0);
  end
endmodule
