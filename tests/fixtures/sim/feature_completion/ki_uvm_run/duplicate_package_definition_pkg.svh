// A package in a guarded header, compiled on its own and included again by
// the testbench, as a verification library's package file is.
`ifndef DUP_PKG_SVH
`define DUP_PKG_SVH
package dup_pkg;
  class seed_map;
    int unsigned count[string];
  endclass
  seed_map table_lookup[string];
  int unsigned created;
  function automatic int unsigned next_seed(string inst);
    seed_map map;
    if (!table_lookup.exists(inst)) begin
      table_lookup[inst] = new;
      created++;
    end
    map = table_lookup[inst];
    if (!map.count.exists(inst)) map.count[inst] = 0;
    map.count[inst]++;
    return map.count[inst];
  endfunction
endpackage
`endif
