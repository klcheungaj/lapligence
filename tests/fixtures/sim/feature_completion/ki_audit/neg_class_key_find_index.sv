// `find_index` of a class-keyed associative array needs a queue of handles as
// its result: a located limit (docs/known_issues.md).
module tb;
  class K; endclass
  int a[K];
  K k;
  initial begin
    k = new;
    a[k] = 1;
    begin
      K q[$];
      q = a.find_index with (item == 1);
      $display("%0d", q.size());
    end
    $finish;
  end
endmodule
