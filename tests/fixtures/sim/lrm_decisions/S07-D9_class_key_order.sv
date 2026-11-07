// S07-D9: associative arrays with a class index traverse the null key first,
// then the keys of one class in creation order. Keys are compared by identity,
// and traversal from a key that is no longer in the array keeps its position.
// 7.8 L8590: "The data type to be used as an index serves as the lookup key
// and imposes an ordering." 7.8.3 L8681-8683: "A null index is valid." "The
// ordering is deterministic but arbitrary."
// Only keys of one class, none created after another object was freed, are
// portable here: any other relative order is left open by the text.
class item_c;
  int id;
  function new(int i);
    id = i;
  endfunction
endclass

module tb;
  item_c keys[5];
  int table_a[item_c];
  item_c key;

  initial begin
    foreach (keys[i]) keys[i] = new(i);
    table_a[keys[3]] = 30;
    table_a[keys[1]] = 10;
    table_a[keys[4]] = 40;
    table_a[null] = 99;
    foreach (table_a[k]) $display("key %0d = %0d", k == null ? -1 : k.id, table_a[k]);
    if (table_a.last(key)) $display("last %0d", key.id);
    table_a.delete(keys[1]);
    key = keys[1];
    if (table_a.next(key)) $display("next from deleted %0d", key.id);
    key = keys[1];
    if (table_a.prev(key)) $display("prev from deleted %0d", key == null ? -1 : key.id);
    key = keys[2];
    if (table_a.exists(key)) $display("unexpected");
    if (table_a.next(key)) $display("next from absent %0d", key.id);
    $finish;
  end
endmodule
