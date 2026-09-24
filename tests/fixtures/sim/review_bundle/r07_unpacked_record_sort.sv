// Like the unpacked-structure map example in IEEE 1800-2009 7.12.2.
module tb;
  typedef struct { byte key; bit flag; } record_t;
  record_t values[0:1];
  initial begin
    values[0].key=8'sd2; values[0].flag=0;
    values[1].key=8'sd1; values[1].flag=1;
    values.sort() with (item.key);
    if (values[0].key !== 8'sd1 || values[0].flag !== 1'b1 ||
        values[1].key !== 8'sd2 || values[1].flag !== 1'b0)
      $fatal(1, "record sort");
    $display("PASS r07_unpacked_record_sort");
    $finish;
  end
endmodule
