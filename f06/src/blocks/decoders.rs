//! This module implements the specific decoders for known data block types.

use std::collections::BTreeMap;

use log::warn;

use crate::prelude::*;
use crate::util::*;

/// Dashes that signal the end of a table in MYSTRAN.
const MYSTRAN_DASHES: &str = "-------------";

/// Returns column indexes for DOFs. Used by a lot of things.
fn dof_cols() -> BTreeMap<Dof, usize> {
  return Dof::all()
    .iter()
    .copied()
    .enumerate()
    .map(|(a, b)| (b, a))
    .collect();
}

/// Returns column indexes for quad stresses and strains.
fn quad_stress_cols() -> BTreeMap<QuadStressField, usize> {
  return [
    QuadStressField::FibreDistance,
    QuadStressField::NormalX,
    QuadStressField::NormalY,
    QuadStressField::ShearXY,
    QuadStressField::Angle,
    QuadStressField::Major,
    QuadStressField::Minor,
    QuadStressField::VonMises
  ].into_iter().enumerate().map(|(a, b)| (b, a)).collect()
}

/// This decodes a displacements block.
pub(crate) struct DisplacementsDecoder {
  /// The flavour of F06 file we're decoding displacements for.
  flavour: Flavour,
  /// The displacement data.
  data: RowBlock<f64, GridPointRef, Dof, { Self::MATWIDTH }>
}

impl BlockDecoder for DisplacementsDecoder {
  type MatScalar = f64;
  type RowIndex = GridPointRef;
  type ColumnIndex = Dof;
  const MATWIDTH: usize = SIXDOF;
  const BLOCK_TYPE: BlockType = BlockType::Displacements;

  fn new(flavour: Flavour) -> Self {
    return Self {
      flavour,
      data: RowBlock::new(dof_cols())
    };
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    return self.data.finalise(Self::BLOCK_TYPE, subcase, line_range);
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    if line.contains(MYSTRAN_DASHES) {
      return LineResponse::Done;
    }
    let dofs: [f64; SIXDOF] = if let Some(arr) = extract_reals(line) {
      arr
    } else {
      return LineResponse::Useless;
    };
    if let Some(gid) = nth_integer(line, 0) {
      self.data.insert_raw((gid as usize).into(), &dofs);
      return LineResponse::Data;
    }
    return LineResponse::Useless;
  }
}

/// The decoder for grid point force balance blocks.
pub(crate) struct GridPointForceBalanceDecoder {
  /// The flavour of F06 file we're decoding displacements for.
  flavour: Flavour,
  /// The current grid point ID.
  gpref: Option<GridPointRef>,
  /// The force balance data.
  data: RowBlock<f64, GridPointForceOrigin, Dof, { Self::MATWIDTH }>
}

impl BlockDecoder for GridPointForceBalanceDecoder {
  type MatScalar = f64;
  type RowIndex = GridPointForceOrigin;
  type ColumnIndex = Dof;
  const MATWIDTH: usize = SIXDOF;
  const BLOCK_TYPE: BlockType = BlockType::GridPointForceBalance;

  fn new(flavour: Flavour) -> Self {
    return Self {
      flavour,
      gpref: None,
      data: RowBlock::new(dof_cols()),
    };
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    return self.data.finalise(Self::BLOCK_TYPE, subcase, line_range);
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    if line.contains(MYSTRAN_DASHES) {
      return LineResponse::Done;
    }
    if line.contains("FORCE BALANCE FOR GRID POINT") {
      self.gpref = nth_integer(line, 0).map(|x| (x as usize).into());
      return LineResponse::Metadata;
    }
    if line.contains("*TOTALS*") {
      return LineResponse::Useless;
    }
    let fo: ForceOrigin = match self.flavour.solver {
      Some(Solver::Mystran) => {
        if line.contains("APPLIED FORCE") {
          ForceOrigin::Load
        } else if line.contains("SPC FORCE") {
          ForceOrigin::SinglePointConstraint
        } else if line.contains("MPC FORCE") {
          ForceOrigin::MultiPointConstraint
        } else if line.contains("ELEM") {
          if let Some(eid) = nth_integer(line, 0) {
            ForceOrigin::Element {
              elem: ElementRef {
                eid: eid as usize,
                etype: nth_etype(line, 0)
              }
            }
          } else {
            return LineResponse::Useless
          }
        } else {
          return LineResponse::Useless;
        }
      },
      Some(Solver::Simcenter) => {
        self.gpref = nth_integer(line, 0).map(|x| (x as usize).into());
        if line.contains("*TOTALS*") {
          return LineResponse::Useless;
        } else if line.contains("APP-LOAD") {
          self.gpref = nth_integer(line, 1).map(|x| (x as usize).into());
          ForceOrigin::Load
        } else if line.contains("F-OF-SPC") {
          ForceOrigin::SinglePointConstraint
        } else if line.contains("F-OF-MPC") {
          ForceOrigin::MultiPointConstraint
        } else {
          let eid = nth_integer(line, 1).map(|x| (x as usize));
          let etype_opt = nth_etype(line, 0);
          match (self.gpref, eid, etype_opt) {
            (Some(_), Some(eid), Some(etype)) => ForceOrigin::Element {
              elem: ElementRef { eid, etype: Some(etype) }
            },
            _ => return LineResponse::Useless
          }
        }
      },
      None => return LineResponse::BadFlavour
    };
    if let Some(gpref) = self.gpref {
      let ri = GridPointForceOrigin {
        grid_point: gpref,
        force_origin: fo,
      };
      if let Some(arr) = extract_reals::<SIXDOF>(line) {
        self.data.insert_raw(ri, &arr);
        return LineResponse::Data;
      } else {
        return LineResponse::BadFlavour;
      }
    }
    return LineResponse::Useless;
  }
}

/// Decoder for the SPC forces block type.
pub(crate) struct SpcForcesDecoder {
  /// The flavour of F06 file we're decoding SPC forces for.
  flavour: Flavour,
  /// The displacement data.
  data: RowBlock<f64, GridPointRef, Dof, { Self::MATWIDTH }>
}

impl BlockDecoder for SpcForcesDecoder {
  type MatScalar = f64;
  type RowIndex = GridPointRef;
  type ColumnIndex = Dof;
  const MATWIDTH: usize = SIXDOF;
  const BLOCK_TYPE: BlockType = BlockType::SpcForces;

  fn new(flavour: Flavour) -> Self {
    return Self {
      flavour,
      data: RowBlock::new(dof_cols())
    };
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    return self.data.finalise(Self::BLOCK_TYPE, subcase, line_range);
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    if line.contains(MYSTRAN_DASHES) {
      return LineResponse::Done;
    }
    let dofs: [f64; SIXDOF] = if let Some(arr) = extract_reals(line) {
      arr
    } else {
      return LineResponse::Useless;
    };
    if let Some(gid) = nth_integer(line, 0) {
      self.data.insert_raw((gid as usize).into(), &dofs);
      return LineResponse::Data;
    }
    return LineResponse::Useless;
  }
}

/// This decodes an applied forces (load vector) block.
pub(crate) struct AppliedForcesDecoder {
  /// The flavour of F06 file we're decoding displacements for.
  flavour: Flavour,
  /// The displacement data.
  data: RowBlock<f64, GridPointRef, Dof, { Self::MATWIDTH }>
}

impl BlockDecoder for AppliedForcesDecoder {
  type MatScalar = f64;
  type RowIndex = GridPointRef;
  type ColumnIndex = Dof;
  const MATWIDTH: usize = SIXDOF;
  const BLOCK_TYPE: BlockType = BlockType::AppliedForces;

  fn new(flavour: Flavour) -> Self {
    return Self {
      flavour,
      data: RowBlock::new(dof_cols())
    };
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    return self.data.finalise(Self::BLOCK_TYPE, subcase, line_range);
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    if line.contains(MYSTRAN_DASHES) {
      return LineResponse::Done;
    }
    let dofs: [f64; Self::MATWIDTH] = if let Some(arr) = extract_reals(line) {
      arr
    } else {
      return LineResponse::Useless;
    };
    if let Some(gid) = nth_integer(line, 0) {
      self.data.insert_raw((gid as usize).into(), &dofs);
      return LineResponse::Data;
    }
    return LineResponse::Useless;
  }
}

/// A decoder for the "stresses in quad elements" table.
pub(crate) struct QuadStressesDecoder {
  /// The flavour of solver we're decoding for.
  flavour: Flavour,
  /// The inner block of data.
  data: RowBlock<f64, ElementSidedPoint, QuadStressField, { Self::MATWIDTH }>,
  /// Current row reference.
  cur_row: Option<<Self as BlockDecoder>::RowIndex>,
  /// Element type, hinted by the header.
  etype: Option<ElementType>
}

impl BlockDecoder for QuadStressesDecoder {
  type MatScalar = f64;
  type RowIndex = ElementSidedPoint;
  type ColumnIndex = QuadStressField;
  const MATWIDTH: usize = 8;
  const BLOCK_TYPE: BlockType = BlockType::QuadStresses;

  fn new(flavour: Flavour) -> Self {
    return Self {
      flavour,
      data: RowBlock::new(quad_stress_cols()),
      cur_row: None,
      etype: None
    };
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    return self.data.finalise(Self::BLOCK_TYPE, subcase, line_range);
  }

  fn good_header(&mut self, header: &str) -> bool {
    self.etype = nth_etype(header, 0);
    if header.contains("THERMAL") || header.contains("ELASTIC") {
      return false;
    }
    return true;
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    // first, take right floats. if there aren't any, we're toast.
    let cols: [f64; Self::MATWIDTH] = if let Some(arr) = extract_reals(line) {
      arr
    } else {
      return LineResponse::Useless;
    };
    // okay, now we get the sided point.
    let fields = line_breakdown(line).collect::<Vec<_>>();
    match self.flavour.solver {
      Some(Solver::Mystran) => {
        // okay, take the first two fields.
        match (fields.first(), fields.get(1)) {
          // eid and point
          (Some(LineField::Integer(eid)), Some(LineField::NoIdea(_))) => {
            self.cur_row.replace(ElementSidedPoint {
              element: ElementRef { eid: *eid as usize, etype: self.etype },
              point: if line.contains("CENTER") {
                ElementPoint::Centroid
              } else if line.contains("GRD") {
                ElementPoint::Corner(
                  if let Some(LineField::Integer(gid)) = fields.get(2) {
                    (*gid as usize).into()
                  } else {
                    warn!("couldn't get elpoint in {}", line);
                    return LineResponse::Abort;
                  }
                )
              } else {
                warn!("couldn't get elpoint in {}", line);
                return LineResponse::Abort;
              },
              side: ElementSide::Bottom,
            });
          },
          // grid point and gid
          (Some(LineField::NoIdea("GRD")), Some(LineField::Integer(gid))) => {
            if let Some(ref mut ri) = self.cur_row {
              ri.point = ElementPoint::Corner((*gid as usize).into());
            } else {
              warn!("grd line without prev row id at {}", line);
              return LineResponse::Abort;
            }
          }
          // centerpoint
          (Some(LineField::NoIdea("CENTER")), _) => {
            if let Some(ref mut ri) = self.cur_row {
              ri.point = ElementPoint::Centroid;
            } else {
              warn!("center line without prev row id at {}", line);
              return LineResponse::Abort;
            }
          },
          // nothing else, flip the side
          _ => {
            if let Some(ref mut ri) = self.cur_row {
              ri.flip_side();
            } else {
              warn!("failed to flip line at {}", line);
              return LineResponse::Abort;
            }
          }
        };
      },
      Some(Solver::Simcenter) => {
        let ints = fields.iter()
          .filter_map(|lf| {
            if let LineField::Integer(i) = lf { Some(i) } else { None }
          }).copied().collect::<Vec<_>>();
        if ints.is_empty() {
          // cont. line
          if let Some(ref mut ri) = self.cur_row {
            ri.flip_side();
          } else {
            warn!("cont line without row index at {}", line);
            return LineResponse::Abort;
          }
        } else {
          // line has row info
          let point = if line.contains("CEN/4") {
            ElementPoint::Centroid
          } else if let Some(gid) = ints.last() {
            ElementPoint::Corner((*gid as usize).into())
          } else {
            warn!("no point at {}", line);
            return LineResponse::Abort;
          };
          let side = ElementSide::Top;
          let eid = if let Some(x) = ints.get(1) {
            *x as usize
          } else if let Some(ri) = self.cur_row {
            ri.element.eid
          } else {
            warn!("no eid at {}", line);
            return LineResponse::Abort;
          };
          self.cur_row.replace(ElementSidedPoint {
            element: ElementRef { eid, etype: self.etype },
            point,
            side
          });
        }
      },
      None => return LineResponse::BadFlavour,
    }
    if let Some(rid) = self.cur_row {
      self.data.insert_raw(rid, &cols);
      return LineResponse::Data;
    } else {
      warn!("found data but couldn't construct row index at {}", line);
      return LineResponse::Abort;
    }
  }
}

/// A decoder for the "strains in quad elements" table. It just uses the same
/// decoder, transparently.
pub(crate) struct QuadStrainsDecoder {
  /// Just use the same decoder.
  inner: QuadStressesDecoder
}

impl BlockDecoder for QuadStrainsDecoder {
  type MatScalar = f64;
  type RowIndex = ElementSidedPoint;
  type ColumnIndex = QuadStrainField;
  const MATWIDTH: usize = 8;
  const BLOCK_TYPE: BlockType = BlockType::QuadStrains;

  fn new(flavour: Flavour) -> Self {
    return Self { inner: QuadStressesDecoder::new(flavour) }
  }

  fn good_header(&mut self, header: &str) -> bool {
    return BlockDecoder::good_header(&mut self.inner, header);
  }

  fn unwrap(
    self,
    subcase: usize,
    line_range: Option<(usize, usize)>
  ) -> FinalBlock {
    let mut fb = self.inner.unwrap(subcase, line_range);
    fb.col_indexes = fb.col_indexes.into_iter()
      .map(|(ci, n)| {
        if let NasIndex::QuadStressField(qss) = ci {
          return (QuadStrainField::from(qss).into(), n);
        } else {
          panic!("bad col index in quadstress");
        }
      })
      .collect();
    fb.block_type = Self::BLOCK_TYPE;
    return fb;
  }

  fn consume(&mut self, line: &str) -> LineResponse {
    return BlockDecoder::consume(&mut self.inner, line);
  }
}
