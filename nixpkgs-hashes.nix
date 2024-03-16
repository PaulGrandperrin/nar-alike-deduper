# NIXPKGS_ALLOW_UNSUPPORTED_SYSTEM=1 NIXPKGS_ALLOW_BROKEN=1 NIXPKGS_ALLOW_INSECURE=1
with builtins; let
  flake_url = let u = getEnv "FLAKE_URL"; in if u == "" then "github:NixOS/nixpkgs/nixos-unstable" else u;
  system = let u = getEnv "NIX_SYSTEM"; in if u == "" then "x86_64-linux" else u;
  flake = getFlake flake_url;
  lib = flake.lib;
  pkgs = flake.legacyPackages.${system};

  evalOr = e: o: let r = tryEval e; in if r.success then r.value else o;
  evalOrTraced = e: o: m: evalOr e (trace m o);
  evalOrNullTraced = e: m: evalOrTraced e null m;
  evalOrFalseTraced = e: m: evalOrTraced e false m;

  allOutputPaths = d: name:
    filter (e: ! isNull e)
      (map
        (o: evalOrNullTraced
          (d.${o}.outPath
            or (trace ("FAILED AT ACCESS OUTPATH: " + name) null)
          )
          ("FAILED AT EVAL OUTPATH: " + name)
        )
        (d.outputs or ["out"])
      )
  ;

  isDerivation = e: name:
    evalOrFalseTraced
      (e.type or null == "derivation")
      ("FAILED AT IS_DERIVATION: " + name)
  ;
  
  isPkgSet = e: name:
    evalOrFalseTraced
      (isAttrs e && e.recurseForDerivations or false)
      ("FAILED AT IS_PKG_SET: " + name)
  ;

  filterDerivationTree = e:
    let f = parent_path: e:
      if isDerivation e parent_path then # derivation
        #allOutputPaths e parent_path
        e
      else
	if isPkgSet e parent_path then # package set
	  lib.filterAttrs
            (n: v: ! isNull v)
            (mapAttrs
              (n: v:
                f (parent_path + "." + n) v
              )
              e
            )
	else # evaluation error or neither derivation nor pkg set
	  null # will be filtered out
    ; in f "pkgs" (e // {recurseForDerivations = true;}) # pkgs doesn't contain recurseForDerivations (probably to prevent infinive recursion) but we need it to start the recursion
  ;

  attrsTreeToFlattenedList = a: p: cond:
    let l = map (n: {name = p ++ [n]; value=a.${n};}) (attrNames a); in
    if any cond (map (e: e.value) l)
    then concatLists (map (e: if cond e.value then attrsTreeToFlattenedList e.value (p ++ e.name) cond else [e]) l)
    else l;

  treeToOutputPathsList = t:
    map ({name, value}:
      {inherit name; value=allOutputPaths value (lib.concatStringsSep "." name);}
    )
    (attrsTreeToFlattenedList (filterDerivationTree t) [] (e: ! (lib.isDerivation e)))
  ;

in with lib;
   treeToOutputPathsList pkgs

