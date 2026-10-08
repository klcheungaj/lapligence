// SIM-018: a 5000-object list is marked without recursion and survives
// repeated collections; dropping its second half leaves exactly the first
// 2500 nodes reachable (SV 8.27). Null handles are never followed.
class link_c;
  int v;
  link_c next;
  function new(int init);
    v = init;
  endfunction
endclass

module tb;
  link_c head;

  function automatic int walk(link_c from);
    int total = 0;
    for (link_c p = from; p != null; p = p.next) total += p.v;
    return total;
  endfunction

  initial begin
    link_c tail;
    head = new(1);
    tail = head;
    for (int i = 2; i <= 5000; i++) begin
      tail.next = new(i);
      tail = tail.next;
      if (i % 100 == 0) #1;
    end
    $display("full %0d", walk(head));
    tail = head;
    for (int i = 1; i < 2500; i++) tail = tail.next;
    tail.next = null;
    tail = null;
    for (int i = 0; i < 20; i++) begin
      link_c g;
      g = new(i);
      g.next = g;
      #1;
    end
    $display("half %0d", walk(head));
    $finish(0);
  end
endmodule
