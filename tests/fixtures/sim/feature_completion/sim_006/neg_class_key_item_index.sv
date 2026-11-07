class C; endclass
module tb; int a[C]; int q[$]; initial q = a.find with (item.index == null); endmodule
