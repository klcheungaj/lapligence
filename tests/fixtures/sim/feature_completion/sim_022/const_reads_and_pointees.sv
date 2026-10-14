// SIM-022-A03 boundary: read-only methods of const containers and strings
// are legal, and a const class handle's object stays writable (only the
// handle is constant), including through a user method named like a
// built-in.
module tb;
  class Bag;
    int q[$];
    function void delete();
      q.delete();
    endfunction
  endclass
  const int limits[$] = '{3, 1, 2};
  const string name = "abc";
  const Bag bag = new;
  initial begin
    bag.q.push_back(4);
    bag.q.push_back(5);
    $display("limits size=%0d sum=%0d first=%0d", limits.size(), limits.sum(), limits[0]);
    $display("name len=%0d upper=%s sub=%s", name.len(), name.toupper(), name.substr(1, 2));
    $display("bag before=%0d", bag.q.size());
    bag.delete();
    $display("bag after=%0d", bag.q.size());
    $finish;
  end
endmodule
